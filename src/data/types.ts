export type TypeCard = {
  key: string;
  title: string;
  subtitle: string;
  yesterdayNew: number;
  highCount: number;
  progress: number;
  tone: string;
};

export type TrendPoint = {
  day: string;
  value: number;
  highlight: boolean;
};

export type RiskClue = {
  id: string;
  partner: string;
  partnerType: string;
  level: number;
  title: string;
  eventDate: string;
  owner: string;
  progress: number;
  status: string;
};

export type EventItem = {
  title: string;
  time: string;
  level: number;
};

export type DashboardSnapshot = {
  userName: string;
  monitorDate: string;
  kpiEffective: number;
  kpiLevel1: number;
  kpiLevel2: number;
  kpiPending: number;
  healthScore: number;
  healthRankLabel: string;
  typeCards: TypeCard[];
  trend: TrendPoint[];
  events: EventItem[];
};
