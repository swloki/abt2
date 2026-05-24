use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::postgres::PgPool;
use tokio::task::JoinHandle;
use tracing::{error, info, instrument, warn};

use super::dead_letter::DeadLetterService;
use super::model::DomainEvent;
use super::registry::EventHandlerRegistry;
use super::repo::DomainEventRepo;
use crate::shared::idempotency::repo::IdempotencyRepo;
use crate::shared::types::error::DomainError;

/// 领域事件处理器 — 后台消费 domain_events 表中待处理事件
pub struct EventProcessor {
    pool: Arc<PgPool>,
    registry: Arc<dyn EventHandlerRegistry>,
    dead_letter: Arc<dyn DeadLetterService>,
    max_retries: i32,
    running: Arc<AtomicBool>,
    last_processed_at: Arc<RwLock<Option<DateTime<Utc>>>>,
    handle: RwLock<Option<JoinHandle<()>>>,
}

impl EventProcessor {
    pub fn new(
        pool: Arc<PgPool>,
        registry: Arc<dyn EventHandlerRegistry>,
        dead_letter: Arc<dyn DeadLetterService>,
        max_retries: i32,
    ) -> Self {
        Self {
            pool,
            registry,
            dead_letter,
            max_retries: max_retries.max(1),
            running: Arc::new(AtomicBool::new(false)),
            last_processed_at: Arc::new(RwLock::new(None)),
            handle: RwLock::new(None),
        }
    }

    /// 启动后台处理任务
    pub fn start(&self) {
        if self.running.load(Ordering::Relaxed) {
            warn!("EventProcessor already running");
            return;
        }
        self.running.store(true, Ordering::Relaxed);
        info!("EventProcessor starting");

        let pool = Arc::clone(&self.pool);
        let registry = Arc::clone(&self.registry);
        let dead_letter = Arc::clone(&self.dead_letter);
        let max_retries = self.max_retries;
        let running = Arc::clone(&self.running);
        let last_processed_at = Arc::clone(&self.last_processed_at);

        let handle = tokio::spawn(async move {
            // 启动时恢复孤儿事件（上次崩溃时卡在 Processing 的）
            if let Ok(mut conn) = pool.acquire().await
                && let Err(e) = DomainEventRepo::reset_stale_processing(&mut conn, 5).await
            {
                warn!("EventProcessor: failed to reset stale processing events: {e}");
            }

            // 带指数退避的 PgListener 连接重试
            let mut pg_listener = {
                let mut backoff = Duration::from_secs(1);
                let max_backoff = Duration::from_secs(30);
                loop {
                    if !running.load(Ordering::Relaxed) {
                        info!("EventProcessor: stopped during PgListener connect");
                        running.store(false, Ordering::Relaxed);
                        return;
                    }
                    match sqlx::postgres::PgListener::connect_with(&pool).await {
                        Ok(l) => break l,
                        Err(e) => {
                            error!("EventProcessor: PgListener connect failed: {e}, retrying in {backoff:?}");
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(max_backoff);
                        }
                    }
                }
            };

            if let Err(e) = pg_listener.listen("domain_event").await {
                error!("EventProcessor: listen failed: {e}");
                running.store(false, Ordering::Relaxed);
                return;
            }

            let mut poll_interval = tokio::time::interval(Duration::from_secs(30));

            while running.load(Ordering::Relaxed) {
                // LISTEN/NOTIFY 等待 + 30s 轮询兜底
                let _notified = tokio::select! {
                    notification = pg_listener.recv() => {
                        match notification {
                            Ok(_n) => true,
                            Err(e) => {
                                error!("EventProcessor: notification error: {e}");
                                false
                            }
                        }
                    }
                    _ = poll_interval.tick() => {
                        true
                    }
                };

                // 处理一批事件
                match Self::process_batch(
                    &pool,
                    &*registry,
                    &*dead_letter,
                    max_retries,
                ).await {
                    Ok(count) => {
                        if count > 0 {
                            info!("EventProcessor: processed {count} events");
                        }
                        let mut lpa = last_processed_at.write().expect("lock poisoned");
                        *lpa = Some(Utc::now());
                    }
                    Err(e) => {
                        error!("EventProcessor: batch error: {e}");
                    }
                }
            }

            // 优雅停机：将卡在 Processing 的事件重置为 Pending
            if let Ok(mut conn) = pool.acquire().await
                && let Err(e) = DomainEventRepo::reset_stale_processing(&mut conn, 0).await
            {
                warn!("EventProcessor: failed to reset processing events on shutdown: {e}");
            }

            info!("EventProcessor stopped");
        });

        *self.handle.write().expect("lock poisoned") = Some(handle);
    }

    /// 停止后台任务
    pub async fn stop(&self) {
        if !self.running.load(Ordering::Relaxed) {
            return;
        }
        info!("EventProcessor stopping");
        self.running.store(false, Ordering::Relaxed);

        let handle = self.handle.write().expect("lock poisoned").take();
        if let Some(h) = handle {
            let _ = h.await;
        }
    }

    /// 是否正在运行
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// 最近一次处理时间
    pub fn last_processed_at(&self) -> Option<DateTime<Utc>> {
        *self.last_processed_at.read().expect("lock poisoned")
    }

    /// 手动重试失败事件
    #[instrument(skip(self))]
    pub async fn retry_failed(&self) -> Result<u64, DomainError> {
        let mut conn = self.pool
            .acquire()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let events = DomainEventRepo::fetch_retryable(
            &mut conn,
            self.max_retries,
            100,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        let count = events.len() as u64;
        for event in &events {
            if let Err(e) = Self::process_single_event(
                &self.pool,
                &*self.registry,
                &*self.dead_letter,
                self.max_retries,
                event,
            ).await {
                error!("retry_failed: event {} error: {e}", event.id);
            }
        }

        Ok(count)
    }

    /// 处理一批事件
    async fn process_batch(
        pool: &PgPool,
        registry: &dyn EventHandlerRegistry,
        dead_letter: &dyn DeadLetterService,
        max_retries: i32,
    ) -> Result<usize, DomainError> {
        // 恢复卡在 Processing 超过 5 分钟的事件（防止 crash 残留）
        if let Ok(mut conn) = pool.acquire().await
            && let Err(e) = DomainEventRepo::reset_stale_processing(&mut conn, 5).await
        {
            warn!("process_batch: failed to reset stale processing: {e}");
        }

        let mut conn = pool
            .acquire()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 先拉取可重试的 Failed 事件，再拉取 Pending
        let mut events = DomainEventRepo::fetch_retryable(&mut conn, max_retries, 50)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let pending = DomainEventRepo::fetch_pending(&mut conn, 100)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        events.extend(pending);

        let count = events.len();
        drop(conn); // 释放 fetch 连接

        for event in &events {
            if let Err(e) = Self::process_single_event(
                pool,
                registry,
                dead_letter,
                max_retries,
                event,
            ).await {
                error!("process_batch: event {} error: {e}", event.id);
            }
        }

        Ok(count)
    }

    /// 处理单个事件
    async fn process_single_event(
        pool: &PgPool,
        registry: &dyn EventHandlerRegistry,
        _dead_letter: &dyn DeadLetterService,
        max_retries: i32,
        event: &DomainEvent,
    ) -> Result<(), DomainError> {
        let mut conn = pool
            .acquire()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        // 幂等检查
        let is_first = IdempotencyRepo::check_and_mark(
            &mut conn,
            event.id,
            "EventProcessor",
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        if !is_first {
            DomainEventRepo::mark_processed(&mut conn, &[event.id])
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            return Ok(());
        }

        // 分发给 handler
        match registry.dispatch(event).await {
            Ok(()) => {
                DomainEventRepo::mark_processed(&mut conn, &[event.id])
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;

                let _ = IdempotencyRepo::mark_processed(
                    &mut conn,
                    event.id,
                    "EventProcessor",
                    None,
                ).await;
            }
            Err(e) => {
                let reason = e.to_string();

                if event.retry_count + 1 >= max_retries {
                    warn!(
                        "Event {} exceeded max retries ({max_retries}), sending to dead letter",
                        event.id
                    );
                    DomainEventRepo::mark_dead_letter(&mut conn, event.id, &reason)
                        .await
                        .map_err(|e2| DomainError::Internal(e2.into()))?;
                } else {
                    DomainEventRepo::mark_failed(&mut conn, event.id, &reason)
                        .await
                        .map_err(|e2| DomainError::Internal(e2.into()))?;
                }
            }
        }

        Ok(())
    }
}
