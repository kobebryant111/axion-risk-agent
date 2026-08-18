import type { DashboardSnapshot } from "./types";

const TYPE_CARDS: DashboardSnapshot["typeCards"] = [
  { key: "loan", title: "助贷机构", subtitle: "投诉 · 约谈 · 现金流", yesterdayNew: 0, highCount: 0, progress: 0, tone: "violet" },
  { key: "guarantee", title: "融资担保", subtitle: "罚单 · 解约 · 股权", yesterdayNew: 0, highCount: 0, progress: 0, tone: "blue" },
  { key: "traffic", title: "流量引流", subtitle: "合作稳定 · 盈利", yesterdayNew: 0, highCount: 0, progress: 0, tone: "amber" },
  { key: "payment", title: "支付机构", subtitle: "政策 · 处罚", yesterdayNew: 0, highCount: 0, progress: 0, tone: "rose" },
  { key: "data", title: "数据服务商", subtitle: "资质 · 政策前瞻", yesterdayNew: 0, highCount: 0, progress: 0, tone: "sky" },
  { key: "collection", title: "催收机构", subtitle: "暴力投诉 · 涉诉", yesterdayNew: 0, highCount: 0, progress: 0, tone: "orange" },
];

/** 无真实数据时的空看板，不含演示线索 */
export function emptyDashboard(): DashboardSnapshot {
  const days = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
  return {
    userName: "风控同学",
    monitorDate: new Date().toISOString().slice(0, 10),
    kpiEffective: 0,
    kpiLevel1: 0,
    kpiLevel2: 0,
    kpiPending: 0,
    healthScore: 100,
    healthRankLabel: "组合风险健康分",
    typeCards: TYPE_CARDS,
    trend: days.map((day, i) => ({
      day,
      value: 0,
      highlight: i === days.length - 1,
    })),
    events: [],
  };
}
