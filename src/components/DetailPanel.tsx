import { useState, useEffect } from "react";
import { Drawer, Descriptions, Tag, Button, Input, message, Space, Alert } from "antd";
import { CopyOutlined } from "@ant-design/icons";
import dayjs from "dayjs";
import { useStore } from "@/stores/data";
import { api } from "@/lib/ipc";
import {
  accountTypeLabel,
  interactionLabel,
  platformLabel,
  INTERACTION_COLORS,
  PLATFORM_COLORS,
} from "@/lib/constants";

export default function DetailPanel() {
  const detailId = useStore((s) => s.detailId);
  const setDetail = useStore((s) => s.setDetail);
  const accounts = useStore((s) => s.accounts);
  const categories = useStore((s) => s.categories);
  const refreshAccounts = useStore((s) => s.refreshAccounts);

  const account = accounts.find((a) => a.id === detailId) ?? null;
  const [note, setNote] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setNote(account?.note ?? "");
  }, [account?.id, account?.note]);

  if (!account) return null;

  const catNames = account.categoryIds
    .map((id) => categories.find((c) => c.id === id)?.name)
    .filter(Boolean);

  const copyLink = async () => {
    if (!account.homepageUrl) return;
    try {
      await navigator.clipboard.writeText(account.homepageUrl);
      message.success("主页链接已复制");
    } catch {
      message.warning("复制失败，请手动选择链接复制");
    }
  };

  const saveNote = async () => {
    setSaving(true);
    try {
      await api.updateNote(account.id, note.trim() || null);
      await refreshAccounts();
      message.success("备注已保存");
    } catch (e) {
      message.error((e as Error).message);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Drawer
      title={account.displayName}
      width={380}
      open={!!detailId}
      onClose={() => setDetail(null)}
    >
      <Descriptions column={1} size="small" bordered>
        <Descriptions.Item label="平台">
          <Tag color={PLATFORM_COLORS[account.platform] ?? "default"}>
            {platformLabel(account.platform)}
          </Tag>
        </Descriptions.Item>
        <Descriptions.Item label="平台账号 ID">
          {account.platformAccountId}
        </Descriptions.Item>
        <Descriptions.Item label="账号类型">
          {accountTypeLabel(account.accountType)}
        </Descriptions.Item>
        <Descriptions.Item label="互动频率">
          <Tag color={INTERACTION_COLORS[account.interactionLevel] ?? "default"}>
            {interactionLabel(account.interactionLevel)}
          </Tag>
        </Descriptions.Item>
        <Descriptions.Item label="关注时间">
          {account.followedAt
            ? dayjs.unix(account.followedAt).format("YYYY-MM-DD HH:mm")
            : "未知"}
        </Descriptions.Item>
        <Descriptions.Item label="主页链接">
          {account.homepageUrl ? (
            <Space>
              <span
                style={{
                  wordBreak: "break-all",
                  color: "#1677ff",
                  fontSize: 12,
                }}
              >
                {account.homepageUrl}
              </span>
              <Button size="small" icon={<CopyOutlined />} onClick={copyLink}>
                复制
              </Button>
            </Space>
          ) : (
            <span className="muted">—</span>
          )}
        </Descriptions.Item>
        <Descriptions.Item label="所属分类">
          {catNames.length ? (
            catNames.map((n) => (
              <Tag key={n} color="blue">
                {n}
              </Tag>
            ))
          ) : (
            <span className="muted">未分类</span>
          )}
        </Descriptions.Item>
      </Descriptions>

      <div style={{ marginTop: 16 }}>
        <div style={{ marginBottom: 6 }}>备注</div>
        <Input.TextArea
          rows={3}
          placeholder="为这个账号记点什么…"
          value={note}
          onChange={(e) => setNote(e.target.value)}
        />
        <Button
          type="primary"
          size="small"
          style={{ marginTop: 8 }}
          loading={saving}
          onClick={saveNote}
        >
          保存备注
        </Button>
      </div>

      <Alert
        style={{ marginTop: 20 }}
        type="info"
        showIcon
        message="想清理这个账号？"
        description="在列表中勾选账号后，点击底部工具栏的「批量取关」即可（当前支持哔哩哔哩，执行前有确认，限速 3~8 秒/个）。"
      />
    </Drawer>
  );
}
