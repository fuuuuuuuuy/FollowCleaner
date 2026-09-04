import { useMemo } from "react";
import { Table, Tag, Empty, Avatar } from "antd";
import { UserOutlined } from "@ant-design/icons";
import type { ColumnsType } from "antd/es/table";
import dayjs from "dayjs";
import type { AccountWithCategories } from "@/types";
import { useStore } from "@/stores/data";
import {
  accountTypeLabel,
  interactionLabel,
  platformLabel,
  INTERACTION_COLORS,
  PLATFORM_COLORS,
} from "@/lib/constants";

export default function AccountTable() {
  const accounts = useStore((s) => s.accounts);
  const loading = useStore((s) => s.loading);
  const selectedIds = useStore((s) => s.selectedIds);
  const toggleSelect = useStore((s) => s.toggleSelect);
  const selectAllFiltered = useStore((s) => s.selectAllFiltered);
  const clearSelection = useStore((s) => s.clearSelection);
  const setDetail = useStore((s) => s.setDetail);
  const detailId = useStore((s) => s.detailId);
  const categories = useStore((s) => s.categories);

  const catName = useMemo(() => {
    const m = new Map(categories.map((c) => [c.id, c.name]));
    return (id: string) => m.get(id) ?? "…";
  }, [categories]);

  const columns: ColumnsType<AccountWithCategories> = [
    {
      title: "账号",
      dataIndex: "displayName",
      render: (_, r) => (
        <div className="account-cell">
          <Avatar size={32} icon={<UserOutlined />} style={{ flexShrink: 0 }} />
          <div className="account-meta">
            <div className="account-name">{r.displayName}</div>
            <div className="account-sub">
              <Tag color={PLATFORM_COLORS[r.platform] ?? "default"} style={{ marginRight: 4 }}>
                {platformLabel(r.platform)}
              </Tag>
              <span className="account-pid">{r.platformAccountId}</span>
            </div>
          </div>
        </div>
      ),
    },
    {
      title: "类型",
      dataIndex: "accountType",
      width: 90,
      render: (t: string) => accountTypeLabel(t),
    },
    {
      title: "互动",
      dataIndex: "interactionLevel",
      width: 90,
      render: (l: string) => (
        <Tag color={INTERACTION_COLORS[l] ?? "default"}>{interactionLabel(l)}</Tag>
      ),
    },
    {
      title: "关注时间",
      dataIndex: "followedAt",
      width: 120,
      render: (t: number | null) =>
        t ? dayjs.unix(t).format("YYYY-MM-DD") : <span className="muted">未知</span>,
    },
    {
      title: "分类",
      dataIndex: "categoryIds",
      render: (ids: string[]) =>
        ids.length ? (
          <span>
            {ids.slice(0, 3).map((id) => (
              <Tag key={id} color="blue">
                {catName(id)}
              </Tag>
            ))}
            {ids.length > 3 && <Tag>+{ids.length - 3}</Tag>}
          </span>
        ) : (
          <span className="muted">未分类</span>
        ),
    },
    {
      title: "备注",
      dataIndex: "note",
      ellipsis: true,
      render: (n: string | null) => n ?? <span className="muted">—</span>,
    },
  ];

  return (
    <Table<AccountWithCategories>
      size="middle"
      rowKey="id"
      columns={columns}
      dataSource={accounts}
      loading={loading}
      pagination={{
        pageSize: 100,
        showSizeChanger: true,
        pageSizeOptions: [50, 100, 200, 500],
        showTotal: (n) => `共 ${n} 个账号`,
      }}
      locale={{
        emptyText: (
          <Empty
            description={
              <span>
                暂无账号数据，点击左上角 <b>导入</b> 按钮开始
              </span>
            }
          />
        ),
      }}
      rowSelection={{
        selectedRowKeys: selectedIds,
        onSelect: (record) => toggleSelect(record.id),
        onSelectAll: (selected) => {
          if (selected) selectAllFiltered();
          else clearSelection();
        },
      }}
      onRow={(record) => ({
        onClick: () => setDetail(record.id),
        style: { cursor: "pointer", background: record.id === detailId ? "#f0f7ff" : undefined },
      })}
    />
  );
}
