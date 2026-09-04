import { useEffect, useRef, useState } from "react";
import { Modal, Steps, Button, Alert, Spin, message, Space } from "antd";
import {
  QrcodeOutlined,
  CheckCircleOutlined,
  CloudDownloadOutlined,
} from "@ant-design/icons";
import QRCode from "qrcode";
import { useStore } from "@/stores/data";
import { api } from "@/lib/ipc";

/**
 * B站账号接入弹窗：扫码登录 → 自动拉取关注列表 → 入库
 * 会话凭证仅存本机，不离开设备。
 */
export default function BiliConnectModal() {
  const open = useStore((s) => s.connectOpen);
  const setOpen = useStore((s) => s.setConnectOpen);
  const biliLoggedIn = useStore((s) => s.biliLoggedIn);
  const biliUname = useStore((s) => s.biliUname);
  const checkBiliStatus = useStore((s) => s.checkBiliStatus);
  const fetchBiliAndImport = useStore((s) => s.fetchBiliAndImport);

  const [step, setStep] = useState(0);
  const [qrImg, setQrImg] = useState<string | null>(null);
  const [pollState, setPollState] = useState<"waiting" | "scanned" | "expired">(
    "waiting",
  );
  const [busy, setBusy] = useState(false);
  const [fetchStats, setFetchStats] = useState<{
    inserted: number;
    updated: number;
    total: number;
  } | null>(null);
  const pollTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const stopped = useRef(false);

  const stopPoll = () => {
    if (pollTimer.current) {
      clearInterval(pollTimer.current);
      pollTimer.current = null;
    }
  };

  const close = () => {
    stopped.current = true;
    stopPoll();
    setOpen(false);
    setTimeout(() => {
      setStep(0);
      setQrImg(null);
      setPollState("waiting");
      setFetchStats(null);
    }, 300);
  };

  // 生成二维码并开始轮询
  const startQr = async () => {
    stopped.current = false;
    setBusy(true);
    setPollState("waiting");
    try {
      const qr = await api.biliQrGenerate();
      const dataUrl = await QRCode.toDataURL(qr.url, {
        width: 240,
        margin: 1,
        color: { dark: "#00a1d6", light: "#ffffff" },
      });
      if (stopped.current) return;
      setQrImg(dataUrl);
      setStep(1);
      startPolling(qr.qrcodeKey);
    } catch (e) {
      message.error((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const startPolling = (key: string) => {
    stopPoll();
    let elapsed = 0;
    pollTimer.current = setInterval(async () => {
      elapsed += 2;
      if (elapsed > 180) {
        // 3 分钟超时
        stopPoll();
        setPollState("expired");
        return;
      }
      try {
        const r = await api.biliQrPoll(key);
        if (r.status === "scanned") setPollState("scanned");
        if (r.status === "expired") {
          stopPoll();
          setPollState("expired");
        }
        if (r.status === "success") {
          stopPoll();
          await checkBiliStatus();
          setStep(2);
          // 自动拉取
          await doFetch();
        }
      } catch (e) {
        // 网络抖动时继续轮询，不打断
        console.warn("poll error", e);
      }
    }, 2000);
  };

  const doFetch = async () => {
    setBusy(true);
    try {
      // 先拉取数量（用于显示），commitImport 会在 store 内完成
      const fetched = await api.biliFetchFollows();
      const result = await fetchBiliAndImport();
      setFetchStats({
        inserted: result.inserted,
        updated: result.updated,
        total: fetched.total,
      });
      setStep(3);
    } catch (e) {
      message.error("拉取关注列表失败：" + (e as Error).message);
      setStep(2);
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    return () => stopPoll();
  }, []);

  // 已登录用户打开弹窗：直接显示同步入口
  useEffect(() => {
    if (open && biliLoggedIn) setStep(3);
  }, [open, biliLoggedIn]);

  return (
    <Modal
      title="接入哔哩哔哩账号"
      open={open}
      onCancel={close}
      width={560}
      footer={
        <Space>
          {step === 0 && (
            <Button type="primary" icon={<QrcodeOutlined />} loading={busy} onClick={startQr}>
              扫码登录
            </Button>
          )}
          {pollState === "expired" && step === 1 && (
            <Button type="primary" loading={busy} onClick={startQr}>
              二维码已过期，点击刷新
            </Button>
          )}
          {step === 2 && (
            <Button type="primary" icon={<CloudDownloadOutlined />} loading={busy} onClick={doFetch}>
              重新拉取关注列表
            </Button>
          )}
          {step === 3 && (
            <Button type="primary" icon={<CheckCircleOutlined />} onClick={close}>
              完成
            </Button>
          )}
        </Space>
      }
    >
      <Steps
        current={step}
        size="small"
        style={{ marginBottom: 20 }}
        items={[
          { title: "扫码登录" },
          { title: "等待确认" },
          { title: "拉取关注" },
          { title: "完成" },
        ]}
      />

      <Alert
        type="info"
        showIcon
        style={{ marginBottom: 16 }}
        message="登录会话仅保存在本机数据库，用于读取你的关注列表和执行取关操作，不会上传到任何服务器。"
      />

      {biliLoggedIn && step >= 2 && (
        <Alert
          type="success"
          showIcon
          style={{ marginBottom: 16 }}
          message={`已登录：${biliUname ?? "B站用户"}`}
        />
      )}

      <div style={{ textAlign: "center", minHeight: 260 }}>
        {step === 0 && (
          <div style={{ paddingTop: 40, color: "#8c8c8c" }}>
            <QrcodeOutlined style={{ fontSize: 48, color: "#00a1d6" }} />
            <p style={{ marginTop: 16 }}>
              点击「扫码登录」，使用手机哔哩哔哩 App 扫码授权
            </p>
            <p style={{ fontSize: 12 }}>
              登录后将自动读取你的关注列表（昵称、UID、关注时间、认证类型）
            </p>
          </div>
        )}

        {step === 1 && (
          <div style={{ paddingTop: 10 }}>
            {qrImg ? (
              <img src={qrImg} alt="bilibili qr" style={{ width: 240, height: 240 }} />
            ) : (
              <Spin />
            )}
            <div style={{ marginTop: 12 }}>
              {pollState === "waiting" && (
                <span style={{ color: "#8c8c8c" }}>
                  请使用 <b style={{ color: "#00a1d6" }}>手机哔哩哔哩 App</b> 扫码
                </span>
              )}
              {pollState === "scanned" && (
                <span style={{ color: "#faad14" }}>
                  已扫码，请在手机上确认登录
                </span>
              )}
              {pollState === "expired" && (
                <span style={{ color: "#ff4d4f" }}>二维码已过期</span>
              )}
            </div>
          </div>
        )}

        {step === 2 && (
          <div style={{ paddingTop: 80 }}>
            <Spin tip="正在从哔哩哔哩拉取关注列表…" size="large">
              <div style={{ height: 60 }} />
            </Spin>
          </div>
        )}

        {step === 3 && fetchStats && (
          <Alert
            type="success"
            showIcon
            style={{ textAlign: "left" }}
            message="关注列表已同步"
            description={
              <span>
                B站账号共关注 <b>{fetchStats.total}</b> 个账号；本次新增{" "}
                <b>{fetchStats.inserted}</b> 个、更新 <b>{fetchStats.updated}</b>{" "}
                个已在库账号。现在可以在列表中筛选、分类、批量取关。
              </span>
            }
          />
        )}
      </div>
    </Modal>
  );
}
