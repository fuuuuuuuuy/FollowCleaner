// 平台与枚举的展示元数据

export const PLATFORM_LABELS: Record<string, string> = {
  wechat: "微信",
  bilibili: "哔哩哔哩",
  xiaohongshu: "小红书",
  weibo: "微博",
  zhihu: "知乎",
  douyin: "抖音",
  manual: "手动录入",
};

export const PLATFORM_COLORS: Record<string, string> = {
  wechat: "#07c160",
  bilibili: "#00a1d6",
  xiaohongshu: "#ff2442",
  weibo: "#e6162d",
  zhihu: "#0066ff",
  douyin: "#000000",
  manual: "#8c8c8c",
};

export function platformLabel(p: string): string {
  return PLATFORM_LABELS[p] ?? p;
}

export const ACCOUNT_TYPE_LABELS: Record<string, string> = {
  personal: "个人",
  official: "官方号",
  subscription_account: "公众号",
  video_account: "视频号",
  brand: "品牌号",
  unknown: "未知类型",
};

export const INTERACTION_LABELS: Record<string, string> = {
  high: "高互动",
  medium: "中互动",
  low: "低互动",
  none: "零互动",
  unknown: "未知",
};

export const INTERACTION_COLORS: Record<string, string> = {
  high: "#52c41a",
  medium: "#faad14",
  low: "#fa8c16",
  none: "#ff4d4f",
  unknown: "#bfbfbf",
};

export function interactionLabel(i: string): string {
  return INTERACTION_LABELS[i] ?? i;
}

export function accountTypeLabel(t: string): string {
  return ACCOUNT_TYPE_LABELS[t] ?? t;
}
