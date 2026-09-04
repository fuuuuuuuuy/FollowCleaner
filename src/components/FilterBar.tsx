import { Input, Select, Segmented, Button, Space, Tooltip } from "antd";
import { SearchOutlined, ReloadOutlined } from "@ant-design/icons";
import { useStore } from "@/stores/data";
import {
  INTERACTION_LABELS,
  PLATFORM_LABELS,
  ACCOUNT_TYPE_LABELS,
} from "@/lib/constants";

const INTERACTION_OPTIONS = Object.entries(INTERACTION_LABELS).map(([v, l]) => ({
  value: v,
  label: l,
}));
const TYPE_OPTIONS = Object.entries(ACCOUNT_TYPE_LABELS).map(([v, l]) => ({
  value: v,
  label: l,
}));

export default function FilterBar() {
  const s = useStore();
  const overview = s.overview;

  const platformOptions = [
    ...(overview?.platforms ?? []).map((p) => ({
      value: p.name,
      label: PLATFORM_LABELS[p.name] ?? p.name,
    })),
    // 已知平台补充（数据库暂无数据时也可选择）
    ...Object.entries(PLATFORM_LABELS)
      .filter(([k]) => k !== "manual")
      .map(([k, l]) => ({ value: k, label: l })),
  ].filter(
    (opt, i, arr) => arr.findIndex((o) => o.value === opt.value) === i,
  );

  return (
    <div className="filter-bar">
      <Space wrap size={8}>
        <Input
          allowClear
          prefix={<SearchOutlined />}
          placeholder="搜索昵称 / 备注 / ID"
          style={{ width: 220 }}
          value={s.search}
          onChange={(e) => s.setFilter({ search: e.target.value })}
        />
        <Select
          mode="multiple"
          allowClear
          placeholder="平台"
          style={{ minWidth: 150, maxWidth: 220 }}
          options={platformOptions}
          value={s.platforms}
          onChange={(v) => s.setFilter({ platforms: v })}
          maxTagCount="responsive"
        />
        <Select
          mode="multiple"
          allowClear
          placeholder="互动频率"
          style={{ minWidth: 150 }}
          options={INTERACTION_OPTIONS}
          value={s.interactionLevels}
          onChange={(v) => s.setFilter({ interactionLevels: v })}
          maxTagCount="responsive"
        />
        <Select
          mode="multiple"
          allowClear
          placeholder="账号类型"
          style={{ minWidth: 150 }}
          options={TYPE_OPTIONS}
          value={s.accountTypes}
          onChange={(v) => s.setFilter({ accountTypes: v })}
          maxTagCount="responsive"
        />
        <Segmented
          value={s.quickFilter}
          onChange={(v) => s.setFilter({ quickFilter: v as "all" | "silent" | "dormant" })}
          options={[
            { label: "全部", value: "all" },
            { label: "零互动", value: "silent" },
            { label: "沉睡号(零互动+关注超1年)", value: "dormant" },
          ]}
        />
        <Select
          style={{ width: 150 }}
          value={`${s.sort}-${s.order}`}
          onChange={(v) => {
            const [sort, order] = v.split("-");
            s.setFilter({ sort, order: order as "asc" | "desc" });
          }}
          options={[
            { value: "followed_at-desc", label: "关注时间 ↓" },
            { value: "followed_at-asc", label: "关注时间 ↑" },
            { value: "interaction-desc", label: "互动频率 高→低" },
            { value: "interaction-asc", label: "互动频率 低→高" },
            { value: "name-asc", label: "昵称 A→Z" },
            { value: "imported-desc", label: "最近导入" },
          ]}
        />
        <Tooltip title="刷新">
          <Button icon={<ReloadOutlined />} onClick={() => s.refreshAccounts()} />
        </Tooltip>
      </Space>
    </div>
  );
}
