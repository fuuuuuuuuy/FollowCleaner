import { useEffect, useMemo, useRef, useState } from "react";
import {
  Modal,
  Alert,
  List,
  Tag,
  Checkbox,
  Button,
  Progress,
  message,
  Space,
} from "antd";
import { WarningOutlined } from "@ant-design/icons";
import { useStore } from "@/stores/data";
import { api } from "@/lib/ipc";
import { platformLabel } from "@/lib/constants";
import type { AccountWithCategories, UnfollowProgress } from "@/types";

/**
 * 批量取消关注弹窗。
 * 当前仅支持哔哩哔哩（L2，扫码登录的会话）。
 * 执行前二次确认；执行中实时显示进度；平台风控触发时熔断停止。
 */
export default function UnfollowModal() {
  const open = useStore((s) => s.unfollowOpen);
  const setOpen = useStore((s) => s.setUnfollowOpen);
  const accounts = useStore((s) => s.accounts);
  const selectedIds = useStore((s) => s.selectedIds);
  const biliLoggedIn = useStore((s) => s.biliLoggedIn);
  const refreshAccounts = useStore((s) => s.refreshAccounts);
  const refreshOverview = useStore((s) => s.refreshOverview);
  const clearSelection = useStore((s) => s.clearSelection);

  const [confirmed, setConfirmed] = useState(false);
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<UnfollowProgress[]>([]);
  const [stopped, setStopped] = useState(false);
  const unlistenRef = useRef<(() => void) | null>(null);

  // 选中账号中可处理（B站 + 在关注中）的部分
  const targets = useMemo(
    () =>
      accounts.filter(
        (a) =>
          selectedIds.includes(a.id) &&
          a.platform === "bilibili" &&
          a.status === "following",
      ),
    [accounts, selectedIds],
  );
  const unsupported = useMemo(
    () =>
      accounts.filter(
        (a) => selectedIds.includes(a.id) && a.platform !== "bilibili",
      ),
    [accounts, selectedIds],
  );

  const reset = () => {
    setConfirmed(false);
    setRunning(false);
    setProgress([]);
    setStopped(false);
  };

  const close = () => {
    if (running) {
      message.warning("取关任务执行中，请等待完成或停止");
      return;
    }
    setOpen(false);
    setTimeout(reset, 300);
  };

  const start = async () => {
    if (targets.length === 0) return;
    setRunning(true);
    setProgress([]);
    setStopped(false);

    const mids = targets.map((a) => a.platformAccountId);

    // 订阅进度事件
    const unlisten = await api.onUnfollowProgress((p) => {
      setProgress((prev) => {
        const next = [...prev];
        const idx = next.findIndex((x) => x.mid === p.mid);
        if (idx >= 0) next[idx] = p;
        else next.push(p);
        return next;
      });
      if (p.stopped) setStopped(true);
      if (p.finished || p.stopped) {
        setRunning(false);
        message[p.stopped ? "warning" : "success"](
          p.stopped ? "任务已被平台风控中断" : "批量取关完成",
        );
        void refreshAccounts();
        void refreshOverview();
        clearSelection();
      }
    });
    unlistenRef.current = unlisten;

    try {
      await api.biliUnfollowBatch(mids);
    } catch (e) {
      message.error("取关任务失败：" + (e as Error).message);
      setRunning(false);
    }
  };

  useEffect(() => {
    return () => {
      unlistenRef.current?.();
    };
  }, []);

  const doneCount = progress.filter((p) => p.success).length;
  const failCount = progress.filter((p) => !p.success).length;
  const percent =
    targets.length > 0 ? Math.round((progress.length / targets.length) * 100) : 0;

  return (
    <Modal
      title={
        <Space>
          <WarningOutlined style={{ color: "#ff4d4f" }} />
          批量取消关注
        </Space>
      }
      open={open}
      onCancel={close}
      width={620}
      footer={
        <Space>
          <Button onClick={close} disabled={running}>
            关闭
          </Button>
          {!running && progress.length === 0 && (
            <Button
              type="primary"
              danger
              disabled={!confirmed || targets.length === 0}
              onClick={start}
            >
              确认取关 {targets.length} 个账号
            </Button>
          )}
        </Space>
      }
    >
      {!biliLoggedIn && (
        <Alert
          type="warning"
          showIcon
          style={{ marginBottom: 12 }}
          message="尚未登录哔哩哔哩账号"
          description="批量取关需要先扫码登录。请关闭本窗口，点击「接入B站账号」完成登录后再试。"
        />
      )}

      {biliLoggedIn && unsupported.length > 0 && (
        <Alert
          type="info"
          showIcon
          style={{ marginBottom: 12 }}
          message={`有 ${unsupported.length} 个选中账号来自其他平台，当前版本暂不支持自动取关（微信/小红书等平台适配器在后续版本提供），已自动跳过。`}
        />
      )}

      {progress.length === 0 ? (
        <>
          <Alert
            type="error"
            showIcon
            style={{ marginBottom: 12 }}
            message="此操作不可撤销"
            description="取关后你将不再收到这些账号的内容更新。工具会以 3~8 秒随机间隔逐个操作（模拟人工、降低风控风险）；如触发平台风控将自动停止。"
          />
          <p style={{ marginBottom: 8 }}>
            即将取消关注以下 <b>{targets.length}</b> 个哔哩哔哩账号：
          </p>
          <List
            size="small"
            bordered
            dataSource={targets}
            style={{ maxHeight: 260, overflow: "auto", marginBottom: 12 }}
            renderItem={(a: AccountWithCategories) => (
              <List.Item>
                <span style={{ flex: 1 }}>
                  <Tag color="cyan">{platformLabel(a.platform)}</Tag>
                  <b>{a.displayName}</b>
                  <span className="muted" style={{ marginLeft: 8, fontSize: 12 }}>
                    UID: {a.platformAccountId}
                  </span>
                </span>
              </List.Item>
            )}
          />
          <Checkbox
            checked={confirmed}
            onChange={(e) => setConfirmed(e.target.checked)}
            disabled={!biliLoggedIn}
          >
            我已知晓取关不可撤销，确认对以上账号执行操作
          </Checkbox>
        </>
      ) : (
        <>
          <Progress
            percent={percent}
            status={stopped ? "exception" : running ? "active" : "success"}
            style={{ marginBottom: 12 }}
          />
          <Alert
            type={stopped ? "warning" : "success"}
            showIcon
            style={{ marginBottom: 12 }}
            message={
              stopped
                ? "任务已停止：触发平台风控或登录失效，剩余账号未处理"
                : `已处理 ${progress.length}/${targets.length}（成功 ${doneCount}，失败 ${failCount}）`
            }
          />
          <List
            size="small"
            bordered
            dataSource={[...progress].reverse()}
            style={{ maxHeight: 320, overflow: "auto" }}
            renderItem={(p) => (
              <List.Item>
                <span style={{ flex: 1 }}>
                  <b>{p.displayName}</b>
                  <span className="muted" style={{ marginLeft: 8, fontSize: 12 }}>
                    {p.message}
                  </span>
                </span>
                <Tag color={p.success ? "success" : "error"}>
                  {p.success ? "已取关" : "失败"}
                </Tag>
              </List.Item>
            )}
          />
          <p className="muted" style={{ fontSize: 12, marginTop: 8 }}>
            每个账号间隔 3~8 秒属正常现象，请勿关闭窗口。
          </p>
        </>
      )}
    </Modal>
  );
}
