import * as XLSX from "xlsx";

export const PARTNER_SHEET = "机构名单";
export const RULE_SHEET = "规则库";

export const PARTNER_HEADERS = [
  "机构全称",
  "简称",
  "集团",
  "统一社会信用代码",
  "机构类型",
  "关联方",
  "合作状态",
] as const;

export const RULE_HEADERS = [
  "规则编号",
  "机构类型",
  "风险点",
  "等级",
  "触发条件",
  "匹配关键词",
  "数据源",
  "法规依据",
  "是否启用",
] as const;

export type TemplateKind = "partners" | "rules";

function normHeader(h: string): string {
  return h.replace(/^\uFEFF/, "").replace(/[\s_\-]/g, "").toLowerCase();
}

function headerMatches(headers: string[], aliases: string[]): boolean {
  const normalized = headers.map(normHeader).filter(Boolean);
  return aliases.some((alias) => {
    const n = normHeader(alias);
    return normalized.some((h) => h === n || h.includes(n));
  });
}

function freezeHeader(ws: XLSX.WorkSheet, widths: number[]) {
  ws["!views"] = [{ state: "frozen", ySplit: 1 }];
  ws["!cols"] = widths.map((wch) => ({ wch }));
}

function downloadWorkbook(
  filename: string,
  sheets: { name: string; rows: (string | number)[][]; widths?: number[] }[],
) {
  const wb = XLSX.utils.book_new();
  for (const sheet of sheets) {
    const ws = XLSX.utils.aoa_to_sheet(sheet.rows);
    const cols = sheet.rows[0]?.length ?? 8;
    freezeHeader(ws, sheet.widths ?? Array.from({ length: cols }, () => 18));
    XLSX.utils.book_append_sheet(wb, ws, sheet.name);
  }
  XLSX.writeFile(wb, filename);
}

export function downloadPartnerTemplate() {
  downloadWorkbook("合作机构名单导入模板.xlsx", [
    {
      name: "填写说明",
      widths: [72],
      rows: [
        ["合作机构名单导入说明（按列填写，无需 AI）"],
        [""],
        ["1. 请只在「机构名单」工作表填写数据，不要修改表头、不要删列。"],
        ["2. 必填列：机构全称、机构类型。"],
        ["3. 机构类型仅允许：助贷、融资担保、引流、支付、数据、催收、金市同业、运营辅助。"],
        ["4. 合作状态：合作中 / 暂停 / 退出；空白视为合作中。"],
        ["5. 关联方可填多个，用顿号、分号或竖线分隔。"],
        ["6. 导入为全量替换，将覆盖当前名单。也可在页面逐条新建。"],
        ["7. 也可另存为 UTF-8 CSV 后导入，表头须与模板一致。"],
      ],
    },
    {
      name: PARTNER_SHEET,
      widths: [28, 14, 16, 24, 12, 24, 10],
      rows: [
        [...PARTNER_HEADERS],
        [
          "某某助贷科技有限公司",
          "某某助贷",
          "某某集团",
          "91110000MA0000000X",
          "助贷",
          "某某担保有限公司",
          "合作中",
        ],
      ],
    },
  ]);
}

/** 导出吹哨报告中的预警事项（Excel） */
export function downloadWhistleAlerts(opts: {
  kindLabel: string;
  period: string;
  title: string;
  createdAt: string;
  summary: string;
  clues: {
    level: number;
    title: string;
    partner: string;
    partnerType?: string;
    eventDate: string;
    status: string;
    sourceSystem?: string;
    credibility?: string;
    summary?: string;
    ruleId?: string;
    legalBasis?: string;
    sourceUrl?: string;
  }[];
  sourceLabel: (source?: string) => string;
}) {
  const safePeriod = opts.period.replace(/[\\/:*?"<>|]/g, "-");
  const rows: (string | number)[][] = [
    [
      "等级",
      "标题",
      "机构",
      "机构类型",
      "事件日期",
      "状态",
      "来源",
      "可信度",
      "摘要",
      "规则编号",
      "法规依据",
      "原文链接",
    ],
    ...opts.clues.map((c) => [
      `L${c.level}`,
      c.title,
      c.partner,
      c.partnerType ?? "",
      c.eventDate,
      c.status,
      opts.sourceLabel(c.sourceSystem),
      c.credibility ? `${c.credibility} 类源` : "",
      c.summary ?? "",
      c.ruleId ?? "",
      c.legalBasis ?? "",
      c.sourceUrl ?? "",
    ]),
  ];

  downloadWorkbook(`风险吹哨${opts.kindLabel}-${safePeriod}-预警事项.xlsx`, [
    {
      name: "报告摘要",
      widths: [16, 56],
      rows: [
        ["报告标题", opts.title],
        ["周期", opts.period],
        ["生成时间", opts.createdAt],
        ["预警条数", opts.clues.length],
        ["摘要", opts.summary],
      ],
    },
    {
      name: "预警事项",
      widths: [8, 40, 22, 12, 12, 10, 10, 10, 40, 14, 20, 36],
      rows,
    },
  ]);
}

export function downloadRuleTemplate() {
  downloadWorkbook("风险规则库导入模板.xlsx", [
    {
      name: "填写说明",
      widths: [72],
      rows: [
        ["风险规则库导入说明（按列填写，无需 AI）"],
        [""],
        ["1. 请只在「规则库」工作表填写数据，不要修改表头、不要删列。"],
        ["2. 必填列：机构类型、风险点、等级。规则编号可空，系统会自动生成。"],
        ["3. 机构类型：通用、助贷、融资担保、引流、支付、数据、催收、金市同业、运营辅助。"],
        ["4. 等级填 1～4（1致命 / 2重大 / 3关注 / 4信息），也可写 L1～L4。"],
        ["5. 匹配关键词可多个，用顿号、分号或竖线分隔。"],
        ["6. 是否启用：是 / 否；空白视为启用。"],
        ["7. 导入为全量替换，将覆盖当前规则库（不再合并内置基线）。也可在页面逐条新建。"],
      ],
    },
    {
      name: RULE_SHEET,
      widths: [16, 12, 28, 8, 28, 22, 16, 22, 10],
      rows: [
        [...RULE_HEADERS],
        [
          "LOAN-COST-24",
          "助贷",
          "综合融资成本超过24%",
          1,
          "宣传或合同口径综合融资成本（含担保费等）超过24%",
          "综合融资成本|年化利率|超24%",
          "官网/合同/舆情",
          "金规〔2025〕9号第6条",
          "是",
        ],
      ],
    },
  ]);
}

function validateHeaders(headers: string[], kind: TemplateKind) {
  if (kind === "partners") {
    if (!headerMatches(headers, ["机构全称", "全称", "机构名称", "name"])) {
      throw new Error("模板缺少「机构全称」列，请先下载固定模板填写后再导入");
    }
    if (!headerMatches(headers, ["机构类型", "类型", "partner_type", "partnertype"])) {
      throw new Error("模板缺少「机构类型」列，请先下载固定模板填写后再导入");
    }
    return;
  }
  if (!headerMatches(headers, ["机构类型", "类型", "partner_type"])) {
    throw new Error("模板缺少「机构类型」列，请先下载固定模板填写后再导入");
  }
  if (!headerMatches(headers, ["风险点", "risk_point", "riskpoint"])) {
    throw new Error("模板缺少「风险点」列，请先下载固定模板填写后再导入");
  }
  if (!headerMatches(headers, ["等级", "level"])) {
    throw new Error("模板缺少「等级」列，请先下载固定模板填写后再导入");
  }
}

function pickSheetName(wb: XLSX.WorkBook, kind: TemplateKind): string {
  const prefer =
    kind === "partners" ? [PARTNER_SHEET, "名单"] : [RULE_SHEET, "规则"];
  for (const label of prefer) {
    const hit = wb.SheetNames.find(
      (n) => n.replace(/\s/g, "") === label || n.includes(label),
    );
    if (hit) return hit;
  }
  const data = wb.SheetNames.find(
    (n) => n !== "填写说明" && !/^WpsReserved/i.test(n),
  );
  if (!data) {
    throw new Error("Excel 中没有可用的数据工作表");
  }
  return data;
}

function sheetToCsv(sheet: XLSX.WorkSheet, kind: TemplateKind): string {
  const rows = XLSX.utils.sheet_to_json<(string | number | null)[]>(sheet, {
    header: 1,
    defval: "",
    raw: false,
  });
  const headerRow = (rows[0] ?? []).map((c) => String(c ?? "").trim());
  if (headerRow.filter(Boolean).length < 2) {
    throw new Error("数据表为空或缺少表头，请使用固定模板");
  }
  validateHeaders(headerRow, kind);
  return XLSX.utils.sheet_to_csv(sheet);
}

/** 把固定模板 xlsx / csv 转成后端按列表头解析的 CSV，不走 LLM */
export async function fileToTemplateCsv(
  file: File,
  kind: TemplateKind,
): Promise<string> {
  const name = file.name.toLowerCase();
  if (
    name.endsWith(".docx") ||
    name.endsWith(".doc") ||
    name.endsWith(".pdf") ||
    name.endsWith(".txt") ||
    name.endsWith(".md") ||
    name.endsWith(".json")
  ) {
    throw new Error("请使用固定 Excel 模板（.xlsx）或 CSV 导入，无需 AI 解析");
  }

  if (name.endsWith(".xlsx") || name.endsWith(".xls")) {
    const buf = await file.arrayBuffer();
    const wb = XLSX.read(buf, { type: "array" });
    if (!wb.SheetNames.length) {
      throw new Error("Excel 中没有工作表");
    }
    const sheetName = pickSheetName(wb, kind);
    const sheet = wb.Sheets[sheetName];
    if (!sheet) {
      throw new Error(`未找到工作表「${sheetName}」`);
    }
    const csv = sheetToCsv(sheet, kind).trim();
    if (!csv) {
      throw new Error("Excel 数据表为空，请检查是否填在「机构名单」或「规则库」工作表");
    }
    return csv;
  }

  if (!name.endsWith(".csv")) {
    throw new Error("仅支持 .xlsx / .xls / .csv，请先下载固定模板");
  }

  const text = (await file.text()).replace(/^\uFEFF/, "").trim();
  if (!text) {
    throw new Error("CSV 为空");
  }
  const headerLine = text.split(/\r?\n/, 1)[0] ?? "";
  const headers = headerLine.split(",").map((h) => h.replace(/^"|"$/g, "").trim());
  validateHeaders(headers, kind);
  return text;
}
