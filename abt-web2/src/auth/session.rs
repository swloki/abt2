use abt_core::shared::identity::model::Claims;

#[derive(Clone)]
pub struct Session {
    pub claims: Claims,
}
