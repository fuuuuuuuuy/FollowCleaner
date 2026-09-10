import { useState } from "react";
import {
  Modal,
  Radio,
  Input,
  Button,
  Table,
  Space,
  message,
  Typography,
} from "antd";
import {
  PlusOutlined,
  DeleteOutlined,
  ThunderboltOutlined,
  EyeOutlined,
} from "@ant-design/icons";
import { useStore } from "@/stores/data";
import { api } from "@/lib/ipc";
import type { AutoCategorizeResult, AutoRuleType } from "@/types";

interface RuleRow {
  key: number;
  categoryName: string;
  keywordsText: string;
}

/**
 * 自动分类弹窗：选择规则 → 预览命中分布 → 应用（挂到「自动分类」父分类下）
 * 关键词规则匹配账号昵称与简介，不区分大小写；重跑幂等，不会重复关联。
 */
export default function AutoCategorizeModal() {
  const open = useStore((s) => s.autoOpen);
  const setOpen = useStore((s) => s.setAutoOpen);
  const refreshCategories = useStore((s) => s.refreshCategories);
  const refreshAccounts = useStore((s) => s.refreshAccounts);
  const refreshOverview = useStore((s) => s.refreshOverview);

  const [ruleType, setRuleType] = useState<AutoRuleType>("smart");
  const [rows, setRows] = useState<RuleRow[]>([
    { key: 1, categoryName: "技术教程", keywordsText: "编程, 开发, 教程" },
  ]);
  const [busy, setBusy] = useState(false);
  const [preview, setPreview] = useState<AutoCategorizeResult | null>(null);
  const [applied, setApplied] = useState<AutoCategorizeResult | null>(null);

  const rowKey = () => Date.now() + Math.random();

  const buildArgs = (dryRun: boolean) => ({
    ruleType,
    keywordRules:
      ruleType === "keywords"
        ? rows.map((r) => ({
            categoryName: r.categoryName,
            keywords: r.keywordsText
              .split(/[,，;；\s]+/)
              .map((k) => k.trim())
              .filter(Boolean),
          }))
        : null,
    dryRun,
  });

  const run = async (dryRun: boolean) => {
    setBusy(true);
    try {
      const result = await api.autoCategorize(buildArgs(dryRun));
      if (dryRun) {
        setPreview(result);
        setApplied(null);
      } else {
        setApplied(result);
        setPreview(result);
        message.success(
          `已自动分类：${result.groups.length} 个分类，新增关联 ${result.totalAssigned} 条（重跑不会重复关联）`,
        );
        await Promise.all([refreshCategories(), refreshAccounts(), refreshOverview()]);
      }
    } catch (e) {
      message.error((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const close = () => {
    setOpen(false);
    setTimeout(() => {
      setPreview(null);
      setApplied(null);
    }, 300);
  };

  return (
    <Modal
      title={
        <Space>
          <ThunderboltOutlined style={{ color: "#00a1d6" }} />
          自动分类
        </Space>
      }
      open={open}
      onCancel={close}
      width={640}
      footer={
        <Space>
          <Button onClick={close}>关闭</Button>
          <Button icon={<EyeOutlined />} loading={busy} onClick={() => run(true)}>
            预览命中分布
          </Button>
          <Button
            type="primary"
            icon={<ThunderboltOutlined />}
            loading={busy}
            onClick={() => run(false)}
          >
            开始分类
          </Button>
        </Space>
      }
    >
      <Radio.Group
        value={ruleType}
        onChange={(e) => {
          setRuleType(e.target.value);
          setPreview(null);
          setApplied(null);
        }}
        style={{ display: "flex", flexDirection: "column", gap: 10, marginBottom: 16 }}
      >
        <Radio value="verifyType">
          按账号认证类型
          <span className="muted" style={{ marginLeft: 8, fontSize: 12 }}>
            机构官方 / 个人认证 / 公众号 / 视频号 / 普通账号
          </span>
        </Radio>
        <Radio value="followAge">
          按关注时长
          <span className="muted" style={{ marginLeft: 8, fontSize: 12 }}>
            1个月内 / 1~3个月 / 3~12个月 / 1年前
          </span>
        </Radio>
        <Radio value="keywords">
          按自定义关键词
          <span className="muted" style={{ marginLeft: 8, fontSize: 12 }}>
            昵称或简介包含关键词即归入该分类，可配多组
          </span>
        </Radio>
      </Radio.Group>

      {ruleType === "keywords" && (
        <div style={{ marginBottom: 16 }}>
          {rows.map((r, i) => (
            <Space key={r.key} style={{ display: "flex", marginBottom: 8 }} align="baseline">
              <Input
                style={{ width: 160 }}
                placeholder={`分类名 ${i + 1}`}
                value={r.categoryName}
                onChange={(e) =>
                  setRows((rs) =>
                    rs.map((x) => (x.key === r.key ? { ...x, categoryName: e.target.value } : x)),
                  )
                }
              />
              <Input
                style={{ width: 300 }}
                placeholder="关键词，逗号分隔（如：编程, 前端, 教程）"
                value={r.keywordsText}
                onChange={(e) =>
                  setRows((rs) =>
                    rs.map((x) => (x.key === r.key ? { ...x, keywordsText: e.target.value } : x)),
                  )
                }
              />
              <Button
                type="text"
                danger
                icon={<DeleteOutlined />}
                disabled={rows.length <= 1}
                onClick={() => setRows((rs) => rs.filter((x) => x.key !== r.key))}
              />
            </Space>
          ))}
          <Button
            type="dashed"
            icon={<PlusOutlined />}
            onClick={() => setRows((rs) => [...rs, { key: rowKey(), categoryName: "", keywordsText: "" }])}
          >
            添加规则
          </Button>
        </div>
      )}

      <Typography.Paragraph type="secondary" style={{ fontSize: 12 }}>
        分类结果统一挂在左侧「自动分类」根分类下（已存在则复用）；账号可同时属于多个分类，
        自动分类不会移除已有的手动分类。建议先「预览」确认分布再应用。
      </Typography.Paragraph>

      {preview && (
        <Table
          size="small"
          dataSource={preview.groups}
          rowKey="categoryName"
          pagination={false}
          caption={
            applied
              ? `应用完成：${applied.groups.length} 个分类，本次新增关联 ${applied.totalAssigned} 条`
              : "预览：各分类命中账号数"
          }
          columns={[
            { title: "分类", dataIndex: "categoryName" },
            {
              title: "账号数",
              dataIndex: "count",
              width: 100,
              render: (n: number) => <b>{n}</b>,
            },
          ]}
        />
      )}
    </Modal>
  );
}
