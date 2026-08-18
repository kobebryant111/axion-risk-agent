import * as XLSX from "xlsx";
import mammoth from "mammoth";

/** 把上传文件转成可给 LLM 的纯文本（支持 docx/xlsx/xls/csv/txt/md/json） */
export async function fileToImportText(file: File): Promise<string> {
  const name = file.name.toLowerCase();

  if (name.endsWith(".docx")) {
    const buf = await file.arrayBuffer();
    const result = await mammoth.extractRawText({ arrayBuffer: buf });
    const text = (result.value || "").replace(/\r/g, "").trim();
    if (text.length < 30) {
      throw new Error("Word 文档提取文本过少，请确认文件非空或另存为 .txt 后重试");
    }
    return text;
  }

  if (name.endsWith(".doc")) {
    throw new Error("暂不支持旧版 .doc，请用 Word 另存为 .docx 后再上传");
  }

  if (name.endsWith(".pdf")) {
    throw new Error("暂不支持 PDF，请导出/另存为 .docx 或 .txt 后上传");
  }

  if (name.endsWith(".xlsx") || name.endsWith(".xls")) {
    const buf = await file.arrayBuffer();
    const wb = XLSX.read(buf, { type: "array" });
    if (!wb.SheetNames.length) {
      throw new Error("Excel 中没有工作表");
    }
    const parts: string[] = [];
    for (const sheetName of wb.SheetNames) {
      if (/^WpsReserved/i.test(sheetName)) continue;
      const sheet = wb.Sheets[sheetName];
      const csv = XLSX.utils.sheet_to_csv(sheet);
      if (csv.trim()) {
        parts.push(`【工作表: ${sheetName}】\n${csv}`);
      }
    }
    const text = parts.join("\n\n").trim();
    if (text.length < 10) {
      throw new Error("Excel 内容为空，请检查文件");
    }
    return text;
  }

  const text = (await file.text()).trim();
  if (text.length < 10) {
    throw new Error("文件内容过短或为空");
  }
  if (text.includes("PK\u0003\u0004") || text.startsWith("PK")) {
    throw new Error("检测到压缩/二进制内容，请直接上传 .docx / .xlsx 或导出为 CSV/TXT");
  }
  return text;
}

export function formatInvokeError(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  if (e && typeof e === "object") {
    const o = e as Record<string, unknown>;
    if (typeof o.message === "string") return o.message;
    try {
      return JSON.stringify(e);
    } catch {
      return String(e);
    }
  }
  return String(e);
}
