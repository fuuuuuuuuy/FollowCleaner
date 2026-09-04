import { useState } from "react";
import { Button, Space, message, Dropdown, Tooltip } from "antd";
import {
  FolderOpenOutlined,
  ExportOutlined,
  CloseOutlined,
  CheckSquareOutlined,
  DisconnectOutlined,
} from "@ant-design/icons";
import { save } from "@tauri-apps/plugin-dialog";
import { useStore } from "@/stores/data";
import { api } from "@/lib/ipc";

export default function BatchBar() {
  const selectedIds = useStore((s) => s.selectedIds);
  const accounts = useStore((s) => s.accounts);
  const selectAllFiltered = useStore((s) => s.selectAllFiltered);
  const clearSelection = useStore((s) => s.clearSelection);
  const setAssignOpen = useStore((s) => s.setAssignOpen);
  const setUnfollowOpen = useStore((s) => s.setUnfollowOpen);
  const biliLoggedIn = useStore((s) => s.biliLoggedIn);
  const setConnectOpen = useStore((s) => s.setConnectOpen);
  const [busy, setBusy] = useState(false);

  if (selectedIds.length === 0) return null;

  // 选中账号中可自动取关的（B站且在关注中）
  const unfollowable = accounts.filter(
    (a) =>
      selectedIds.includes(a.id) &&
      a.platform === "bilibili" &&
      a.status === "following",
  );

  const onUnfollowClick = () => {
    if (!biliLoggedIn) {
      message.info("请先点击右上角「接入B站账号」扫码登录");
      setConnectOpen(true);
      return;
    }
    if (unfollowable.length === 0) {
      message.info("选中的账号中没有可自动取关的B站账号（其他平台将在后续版本支持）");
      return;
    }
    setUnfollowOpen(true);
  };

  const doExport = async (format: "csv" | "json", scope: "selected" | "all") => {
    const ext = format;
    const path = await save({
      title: scope === "selected" ? "导出选中账号" : "导出全部账号",
      defaultPath: `followcleaner-export.${ext}`,
      filters: [{ name: format.toUpperCase(), extensions: [ext] }],
    });
    if (!path) return;
    setBusy(true);
    try {
      const n = await api.exportAccounts(
        path,
        format,
        scope === "selected" ? selectedIds : null,
      );
      message.success(`已导出 ${n} 个账号到：${path}`);
    } catch (e) {
      message.error((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const exportMenu = [
    { key: "csv-sel", label: "导出选中为 CSV", onClick: () => doExport("csv", "selected") },
    { key: "json-sel", label: "导出选中为 JSON", onClick: () => doExport("json", "selected") },
    { type: "divider" as const },
    { key: "csv-all", label: "导出全部为 CSV", onClick: () => doExport("csv", "all") },
    { key: "json-all", label: "导出全部为 JSON", onClick: () => doExport("json", "all") },
  ];

  return (
    <div className="batch-bar">
      <Space>
        <b>已选 {selectedIds.length}</b> 个账号（当前筛选共 {accounts.length} 个）
      </Space>
      <Space>
        <Button size="small" icon={<CheckSquareOutlined />} onClick={selectAllFiltered}>
          全选筛选结果
        </Button>
        <Button
          size="small"
          type="primary"
          icon={<FolderOpenOutlined />}
          onClick={() => setAssignOpen(true)}
        >
          分配分类
        </Button>
        <Tooltip
          title={
            unfollowable.length > 0
              ? `对 ${unfollowable.length} 个B站账号执行取关（3~8秒/个）`
              : "仅支持哔哩哔哩账号；微信/小红书等平台适配器在后续版本"
          }
        >
          <Button
            size="small"
            danger
            icon={<DisconnectOutlined />}
            onClick={onUnfollowClick}
          >
            批量取关{unfollowable.length > 0 ? `(${unfollowable.length})` : ""}
          </Button>
        </Tooltip>
        <Dropdown menu={{ items: exportMenu }}>
          <Button size="small" icon={<ExportOutlined />} loading={busy}>
            导出
          </Button>
        </Dropdown>
        <Button size="small" icon={<CloseOutlined />} onClick={clearSelection}>
          取消选择
        </Button>
      </Space>
    </div>
  );
}
