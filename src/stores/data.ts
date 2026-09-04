import { create } from "zustand";
import { api } from "@/lib/ipc";
import type {
  AccountFilter,
  AccountWithCategories,
  Category,
  ImportResult,
  NormalizedAccount,
  OverviewCounts,
} from "@/types";

// 虚拟节点 id：全部 / 未分类
export const ALL_ID = "__all__";
export const UNCATEGORIZED_ID = "__uncategorized__";

const YEAR_SECONDS = 365 * 24 * 3600;

interface DataState {
  // 数据
  categories: Category[];
  counts: Record<string, number>;
  accounts: AccountWithCategories[];
  overview: OverviewCounts | null;
  loading: boolean;
  error: string | null;

  // 筛选条件
  search: string;
  platforms: string[];
  interactionLevels: string[];
  accountTypes: string[];
  categoryId: string; // ALL_ID | UNCATEGORIZED_ID | 真实分类 id
  quickFilter: "all" | "silent" | "dormant"; // 全部 / 零互动 / 沉睡(零互动+关注超1年)
  sort: string;
  order: "asc" | "desc";

  // 选择与详情
  selectedIds: string[];
  detailId: string | null;

  // 弹窗
  importOpen: boolean;
  assignOpen: boolean;
  connectOpen: boolean;
  unfollowOpen: boolean;

  // B站会话
  biliLoggedIn: boolean;
  biliUname: string | null;

  // 动作
  init: () => Promise<void>;
  refreshCategories: () => Promise<void>;
  refreshAccounts: () => Promise<void>;
  refreshOverview: () => Promise<void>;
  setFilter: (patch: Partial<Pick<DataState, "search" | "platforms" | "interactionLevels" | "accountTypes" | "categoryId" | "quickFilter" | "sort" | "order">>) => void;
  toggleSelect: (id: string) => void;
  selectAllFiltered: () => void;
  clearSelection: () => void;
  setDetail: (id: string | null) => void;
  setImportOpen: (open: boolean) => void;
  setAssignOpen: (open: boolean) => void;
  setConnectOpen: (open: boolean) => void;
  setUnfollowOpen: (open: boolean) => void;
  checkBiliStatus: () => Promise<void>;
  biliLogout: () => Promise<void>;
  fetchBiliAndImport: () => Promise<ImportResult>;

  createCategory: (name: string, parentId: string | null) => Promise<void>;
  renameCategory: (id: string, name: string) => Promise<void>;
  deleteCategory: (id: string) => Promise<void>;
  moveCategory: (id: string, newParentId: string | null, position: number) => Promise<void>;
  assignCategories: (categoryIds: string[]) => Promise<void>;
  commitImport: (accounts: NormalizedAccount[]) => Promise<ImportResult>;
}

function buildFilter(s: DataState): AccountFilter {
  const filter: AccountFilter = {
    search: s.search || null,
    platforms: s.platforms,
    interactionLevels: s.interactionLevels,
    accountTypes: s.accountTypes,
    sort: s.sort,
    order: s.order,
    statuses: ["following"],
  };
  if (s.categoryId === UNCATEGORIZED_ID) {
    filter.uncategorized = true;
  } else if (s.categoryId !== ALL_ID) {
    filter.categoryId = s.categoryId;
  }
  if (s.quickFilter === "silent") {
    filter.interactionLevels = ["none"];
  } else if (s.quickFilter === "dormant") {
    filter.interactionLevels = ["none"];
    filter.followedBefore = Math.floor(Date.now() / 1000) - YEAR_SECONDS;
  }
  return filter;
}

export const useStore = create<DataState>((set, get) => ({
  categories: [],
  counts: {},
  accounts: [],
  overview: null,
  loading: false,
  error: null,

  search: "",
  platforms: [],
  interactionLevels: [],
  accountTypes: [],
  categoryId: ALL_ID,
  quickFilter: "all",
  sort: "followed_at",
  order: "desc",

  selectedIds: [],
  detailId: null,
  importOpen: false,
  assignOpen: false,
  connectOpen: false,
  unfollowOpen: false,
  biliLoggedIn: false,
  biliUname: null,

  init: async () => {
    set({ loading: true, error: null });
    try {
      await Promise.all([
        get().refreshCategories(),
        get().refreshOverview(),
        get().checkBiliStatus(),
      ]);
      await get().refreshAccounts();
    } catch (e) {
      set({ error: (e as Error).message });
    } finally {
      set({ loading: false });
    }
  },

  refreshCategories: async () => {
    const [categories, counts] = await Promise.all([
      api.listCategories(),
      api.categoryCounts(),
    ]);
    set({ categories, counts });
  },

  refreshAccounts: async () => {
    set({ loading: true });
    try {
      const filter = buildFilter(get());
      const accounts = await api.queryAccounts(filter);
      set({ accounts, error: null });
      // 清理已不在结果中的选中项
      const ids = new Set(accounts.map((a) => a.id));
      set((s) => ({
        selectedIds: s.selectedIds.filter((id) => ids.has(id)),
        detailId: s.detailId && ids.has(s.detailId) ? s.detailId : null,
      }));
    } catch (e) {
      set({ error: (e as Error).message });
    } finally {
      set({ loading: false });
    }
  },

  refreshOverview: async () => {
    const overview = await api.overviewCounts();
    set({ overview });
  },

  setFilter: (patch) => {
    set(patch);
    void get().refreshAccounts();
  },

  toggleSelect: (id) =>
    set((s) => ({
      selectedIds: s.selectedIds.includes(id)
        ? s.selectedIds.filter((x) => x !== id)
        : [...s.selectedIds, id],
    })),

  selectAllFiltered: () => set((s) => ({ selectedIds: s.accounts.map((a) => a.id) })),
  clearSelection: () => set({ selectedIds: [] }),
  setDetail: (id) => set({ detailId: id }),
  setImportOpen: (open) => set({ importOpen: open }),
  setAssignOpen: (open) => set({ assignOpen: open }),
  setConnectOpen: (open) => set({ connectOpen: open }),
  setUnfollowOpen: (open) => set({ unfollowOpen: open }),

  checkBiliStatus: async () => {
    try {
      const st = await api.biliStatus();
      set({ biliLoggedIn: st.loggedIn, biliUname: st.uname });
    } catch {
      // 桌面外环境静默
    }
  },

  biliLogout: async () => {
    await api.biliLogout();
    set({ biliLoggedIn: false, biliUname: null });
  },

  fetchBiliAndImport: async () => {
    const fetched = await api.biliFetchFollows();
    const result = await get().commitImport(fetched.accounts);
    return result;
  },

  createCategory: async (name, parentId) => {
    await api.createCategory(name, parentId);
    await get().refreshCategories();
  },
  renameCategory: async (id, name) => {
    await api.renameCategory(id, name);
    await get().refreshCategories();
  },
  deleteCategory: async (id) => {
    await api.deleteCategory(id);
    await get().refreshCategories();
    if (get().categoryId === id) set({ categoryId: ALL_ID });
    await get().refreshAccounts();
  },
  moveCategory: async (id, newParentId, position) => {
    await api.moveCategory(id, newParentId, position);
    await get().refreshCategories();
  },

  assignCategories: async (categoryIds) => {
    const { selectedIds } = get();
    await api.assignCategories(selectedIds, categoryIds);
    set({ assignOpen: false, selectedIds: [] });
    await Promise.all([get().refreshAccounts(), get().refreshCategories()]);
  },

  commitImport: async (accounts) => {
    const result = await api.commitImport(accounts);
    await Promise.all([
      get().refreshCategories(),
      get().refreshAccounts(),
      get().refreshOverview(),
    ]);
    return result;
  },
}));

export { buildFilter };
