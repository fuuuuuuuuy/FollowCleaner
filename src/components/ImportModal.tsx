import { useState } from "react";
import {
  Modal,
  Steps,
  Button,
  Space,
  Radio,
  Table,
  Statistic,
  Row,
  Col,
  Alert,
  message,
} from "antd";
import {
  DownloadOutlined,
  FileSearchOutlined,
  CloudUploadOutlined,
  CheckCircleOutlined,
} from "@ant-design/icons";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { useStore } from "@/stores/data";
import { api } from "@/lib/ipc";
import { platformLabel } from "@/lib/constants";
import type { ParseResult } from "@/types";

export default function ImportModal() {
  const open = useStore((s) => s.importOpen);
  const setOpen = useStore((s) => s.setImportOpen);
  const commitImport = useStore((s) => s.commitImport);

  const [step, setStep] = useState(0);
  const [format, setFormat] = useState<"csv" | "json">("csv");
  const [filePath, setFilePath] = useState<string | null>(null);
  const [parseResult, setParseResult] = useState<ParseResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [importStats, setImportStats] = useState<{ inserted: number; updated: number } | null>(
    null,
  );

  const reset = () => {
    setStep(0);
    setFilePath(null);
    setParseResult(null);
    setImportStats(null);
  };

  const close = () => {
    setOpen(false);
    setTimeout(reset, 300);
  };

  // Step 0: 下载模板
  const downloadTemplate = async () => {
    const path = await saveDialog({
      title: "保存导入模板",
      defaultPath: `followcleaner-template.${format}`,
      filters: [{ name: format.toUpperCase(), extensions: [format] }],
    });
    if (!path) return;
    setBusy(true);
    try {
      await api.writeTemplate(path, format);
      message.success(`模板已保存：${path}`);
    } catch (e) {
      message.error((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  // Step 1: 选择并解析文件
  const pickAndParse = async () => {
    const path = await openDialog({
      title: "选择关注列表文件",
      multiple: false,
      filters: [
        { name: "支持的文件", extensions: ["csv", "json"] },
        { name: "CSV", extensions: ["csv"] },
        { name: "JSON", extensions: ["json"] },
      ],
    });
    if (!path || Array.isArray(path)) return;
    setBusy(true);
    try {
      const result = await api.parseImportFile(path);
      setFilePath(path);
      setParseResult(result);
      if (result.validCount > 0) {
        setStep(2);
      } else {
        message.warning("文件中没有可导入的有效数据，请查看错误信息");
      }
    } catch (e) {
      message.error((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  // Step 2 → 3: 提交导入
  const doCommit = async () => {
    if (!parseResult) return;
    setBusy(true);
    try {
      const stats = await commitImport(parseResult.accounts);
      setImportStats(stats);
      setStep(3);
    } catch (e) {
      message.error((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title="导入关注列表"
      open={open}
      onCancel={close}
      width={720}
      footer={
        <Space>
          {step > 0 && step < 3 && (
            <Button onClick={() => setStep(step - 1)}>上一步</Button>
          )}
          {step === 0 && (
            <Button type="primary" onClick={() => setStep(1)}>
              我已准备好文件，下一步
            </Button>
          )}
          {step === 1 && (
            <Button
              type="primary"
              icon={<FileSearchOutlined />}
              loading={busy}
              onClick={pickAndParse}
            >
              选择文件并校验
            </Button>
          )}
          {step === 2 && (
            <Button type="primary" icon={<CloudUploadOutlined />} loading={busy} onClick={doCommit}>
              确认导入 {parseResult?.validCount} 个账号
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
          { title: "获取模板" },
          { title: "选择文件" },
          { title: "校验预览" },
          { title: "完成" },
        ]}
      />

      {step === 0 && (
        <Space direction="vertical" style={{ width: "100%" }}>
          <Alert
            type="info"
            showIcon
            message="V0.1 使用手动导入：所有数据仅保存在本机，不上传任何服务器。"
            description="在模板中填入各平台的关注账号信息（platform 字段如 wechat / bilibili / xiaohongshu / weibo / zhihu 等），每行一个账号。模板中的 example 示例行会在导入时自动跳过。"
          />
          <Radio.Group
            value={format}
            onChange={(e) => setFormat(e.target.value)}
            optionType="button"
            buttonStyle="solid"
            options={[
              { label: "CSV 模板（Excel 可编辑）", value: "csv" },
              { label: "JSON 模板", value: "json" },
            ]}
          />
          <Button icon={<DownloadOutlined />} loading={busy} onClick={downloadTemplate}>
            下载{format.toUpperCase()}模板
          </Button>
        </Space>
      )}

      {step === 1 && (
        <Space direction="vertical" style={{ width: "100%" }}>
          <p>选择你整理好的 CSV / JSON 文件，系统将在本地校验数据格式。</p>
          {filePath && (
            <Alert type="success" showIcon message={`已选择：${filePath}`} />
          )}
        </Space>
      )}

      {step === 2 && parseResult && (
        <Space direction="vertical" style={{ width: "100%" }} size="middle">
          <Row gutter={16}>
            <Col span={6}>
              <Statistic title="总行数" value={parseResult.totalRows} />
            </Col>
            <Col span={6}>
              <Statistic title="可导入" value={parseResult.validCount} valueStyle={{ color: "#3f8600" }} />
            </Col>
            <Col span={6}>
              <Statistic title="跳过（示例行）" value={parseResult.skippedCount} />
            </Col>
            <Col span={6}>
              <Statistic
                title="错误行"
                value={parseResult.errors.length}
                valueStyle={{ color: parseResult.errors.length ? "#cf1322" : undefined }}
              />
            </Col>
          </Row>

          {parseResult.errors.length > 0 && (
            <>
              <Alert
                type="error"
                showIcon
                message={`${parseResult.errors.length} 行数据有问题，这些行不会被导入`}
              />
              <Table
                size="small"
                pagination={{ pageSize: 5 }}
                rowKey="row"
                dataSource={parseResult.errors}
                columns={[
                  { title: "行号", dataIndex: "row", width: 80 },
                  { title: "问题", dataIndex: "message" },
                ]}
              />
            </>
          )}

          <Table
            size="small"
            pagination={{ pageSize: 5 }}
            rowKey="platformAccountId"
            dataSource={parseResult.accounts.slice(0, 50)}
            columns={[
              { title: "平台", dataIndex: "platform", width: 100, render: (p: string) => platformLabel(p) },
              { title: "昵称", dataIndex: "displayName" },
              { title: "账号ID", dataIndex: "platformAccountId", width: 140 },
              { title: "互动", dataIndex: "interactionLevel", width: 90 },
            ]}
          />
          <p className="muted" style={{ fontSize: 12 }}>
            重复导入同一账号会自动更新（按平台 + 账号 ID 去重），不会产生重复数据。
          </p>
        </Space>
      )}

      {step === 3 && importStats && (
        <Alert
          type="success"
          showIcon
          message="导入完成"
          description={
            <span>
              新增 <b>{importStats.inserted}</b> 个账号，更新{" "}
              <b>{importStats.updated}</b> 个已有账号。现在可以在左侧分类树和列表中管理它们了。
            </span>
          }
        />
      )}
    </Modal>
  );
}
