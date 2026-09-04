// 与 Rust 端 models.rs 对应的类型契约（camelCase）

export interface Account {
  id: string;
  platform: string;
  platformAccountId: string;
  displayName: string;
  avatarPath: string | null;
  accountType: string;
  homepageUrl: string | null;
  followedAt: number | null;
  interactionLevel: string;
  interactionMetrics: string | null;
  status: string;
  note: string | null;
  importedAt: number;
  updatedAt: number;
}

export interface AccountWithCategories extends Account {
  categoryIds: string[];
}

export interface Category {
  id: string;
  name: string;
  parentId: string | null;
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
}

export interface AccountFilter {
  search?: string | null;
  platforms?: string[];
  accountTypes?: string[];
  interactionLevels?: string[];
  statuses?: string[];
  followedAfter?: number | null;
  followedBefore?: number | null;
  categoryId?: string | null;
  uncategorized?: boolean | null;
  sort?: string | null;
  order?: string | null;
}

export interface NormalizedAccount {
  platform: string;
  platformAccountId: string;
  displayName: string;
  accountType: string;
  homepageUrl: string | null;
  followedAt: number | null;
  interactionLevel: string;
  note: string | null;
}

export interface RowError {
  row: number;
  message: string;
}

export interface ParseResult {
  totalRows: number;
  validCount: number;
  skippedCount: number;
  errors: RowError[];
  accounts: NormalizedAccount[];
}

export interface ImportResult {
  inserted: number;
  updated: number;
}

export interface NameCount {
  name: string;
  count: number;
}

export interface OverviewCounts {
  total: number;
  uncategorized: number;
  platforms: NameCount[];
  interaction: NameCount[];
}

// IPC 错误结构：{ code, message }
export interface IpcError {
  code: string;
  message: string;
}

// ---- B站适配器 ----
export interface BiliQrStart {
  qrcodeKey: string;
  url: string;
}

export interface BiliQrPoll {
  status: "waiting" | "scanned" | "expired" | "success";
  uname: string | null;
}

export interface BiliStatus {
  loggedIn: boolean;
  uname: string | null;
}

export interface BiliFetchResult {
  total: number;
  accounts: NormalizedAccount[];
}

export interface UnfollowProgress {
  total: number;
  done: number;
  mid: string;
  displayName: string;
  success: boolean;
  message: string;
  finished: boolean;
  stopped: boolean;
}
