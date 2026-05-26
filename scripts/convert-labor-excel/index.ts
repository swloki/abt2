/**
 * 人工成本 Excel 转换脚本（统一版）
 *
 * 读取同目录下的 config.toml 配置，将各车间的原始宽表 Excel
 * 转为人工成本导入用的长表 Excel。
 *
 * 用法：
 *   bun run scripts/convert-labor-excel              # 转换所有车间
 *   bun run scripts/convert-labor-excel 电源           # 只转换电源车间
 *   bun run scripts/convert-labor-excel 模组           # 只转换模组车间
 */

import * as XLSX from "xlsx";
import path from "path";
import fs from "fs";
import TOML from "smol-toml";

// ─── 配置类型 ──────────────────────────────────────────────────────────

interface ProcessConfig {
  code: string;
  name: string;
  price_column: string;
  qty_column?: string;
}

interface WorkshopConfig {
  name: string;
  input_file: string;
  output_file: string;
  header_row: number;
  product_code_column: string;
  processes: ProcessConfig[];
}

interface Config {
  workshops: WorkshopConfig[];
}

// ─── 转换逻辑 ──────────────────────────────────────────────────────────

const EXPORT_HEADERS = ["产品编码", "工序编码", "工序名称", "单价", "数量", "排序", "备注"];

function convertWorkshop(ws: WorkshopConfig) {
  console.log(`\n${"=".repeat(60)}`);
  console.log(`  车间: ${ws.name}`);
  console.log(`${"=".repeat(60)}`);

  const exeDir = path.dirname(process.execPath);
  const inputFile = path.resolve(exeDir, ws.input_file);
  const outputFile = path.resolve(exeDir, ws.output_file);

  if (!fs.existsSync(inputFile)) {
    console.error(`  输入文件不存在: ${inputFile}`);
    return;
  }

  const buf = fs.readFileSync(inputFile);
  const wb = XLSX.read(buf, { type: "buffer" });
  const sheet = wb.Sheets[wb.SheetNames[0]];
  const rows: any[][] = XLSX.utils.sheet_to_json(sheet, { header: 1 });

  if (rows.length === 0) {
    console.error("  Excel 为空");
    return;
  }

  const header = rows[ws.header_row] as string[];
  console.log(`  列头: ${header.join(", ")}`);

  const colIndex: Record<string, number> = {};
  header.forEach((h, i) => { colIndex[String(h).trim()] = i; });

  // 验证列存在
  for (const proc of ws.processes) {
    if (!(proc.price_column in colIndex)) {
      console.error(`  缺少单价列: ${proc.price_column}`);
      return;
    }
    if (proc.qty_column && !(proc.qty_column in colIndex)) {
      console.error(`  缺少数量列: ${proc.qty_column}`);
      return;
    }
  }

  const outRows: any[][] = [EXPORT_HEADERS];
  let skipped = 0;

  for (let r = ws.header_row + 1; r < rows.length; r++) {
    const row = rows[r];
    const productCode = String(row[colIndex[ws.product_code_column]] ?? "").trim();
    if (!productCode) continue;

    let sortOrder = 1;

    for (const proc of ws.processes) {
      const rawPrice = row[colIndex[proc.price_column]];
      const unitPrice = Number(rawPrice);

      if (rawPrice == null || unitPrice <= 0) {
        outRows.push([productCode, proc.code, proc.name, 0.01, 0, sortOrder++, "无此工序"]);
        skipped++;
        continue;
      }

      const quantity = proc.qty_column
        ? Number(row[colIndex[proc.qty_column]] ?? 1)
        : 1;

      outRows.push([productCode, proc.code, proc.name, unitPrice, quantity, sortOrder++, ""]);
    }
  }

  // 写出
  fs.mkdirSync(path.dirname(outputFile), { recursive: true });

  const outWb = XLSX.utils.book_new();
  const outWs = XLSX.utils.aoa_to_sheet(outRows);
  outWs["!cols"] = [
    { wch: 18 }, // 产品编码
    { wch: 10 }, // 工序编码
    { wch: 14 }, // 工序名称
    { wch: 12 }, // 单价
    { wch: 8 },  // 数量
    { wch: 6 },  // 排序
    { wch: 20 }, // 备注
  ];

  XLSX.utils.book_append_sheet(outWb, outWs, "人工成本");
  const outBuf = XLSX.write(outWb, { type: "buffer", bookType: "xlsx" }) as Buffer;
  fs.writeFileSync(outputFile, outBuf);

  const dataRows = outRows.length - 1;
  const products = new Set(outRows.slice(1).map(r => r[0]));
  console.log(`  产品数: ${products.size}`);
  console.log(`  工序行数: ${dataRows}`);
  console.log(`  无此工序: ${skipped}`);
  console.log(`  输出: ${outputFile}`);
}

// ─── 入口 ──────────────────────────────────────────────────────────────

function main() {
  const exeDir = path.dirname(process.execPath);
  const configPath = path.resolve(exeDir, "config.toml");

  if (!fs.existsSync(configPath)) {
    console.error(`配置文件不存在: ${configPath}`);
    process.exit(1);
  }

  const configText = fs.readFileSync(configPath, "utf-8");
  const config = TOML.parse(configText) as unknown as Config;

  const filter = process.argv[2]?.trim();
  const workshops = filter
    ? config.workshops.filter(w => w.name.includes(filter))
    : config.workshops;

  if (workshops.length === 0) {
    console.error(`未找到匹配的车间配置: ${filter ?? "(无)"}`);
    console.error(`可用车间: ${config.workshops.map(w => w.name).join(", ")}`);
    process.exit(1);
  }

  console.log(`匹配到 ${workshops.length} 个车间: ${workshops.map(w => w.name).join(", ")}`);

  for (const ws of workshops) {
    convertWorkshop(ws);
  }

  console.log("\n全部转换完成。");
}

main();
