import { useEffect } from "react";
import { Layout, Button, Alert, Spin, Tag, Space, Tooltip } from "antd";
import {
  ImportOutlined,
  ClearOutlined,
  ApiOutlined,
  LogoutOutlined,
} from "@ant-design/icons";
import { useStore } from "@/stores/data";
import CategoryTree from "@/components/CategoryTree";
import FilterBar from "@/components/FilterBar";
import AccountTable from "@/components/AccountTable";
import DetailPanel from "@/components/DetailPanel";
import BatchBar from "@/components/BatchBar";
import ImportModal from "@/components/ImportModal";
import AssignCategoryModal from "@/components/AssignCategoryModal";
import BiliConnectModal from "@/components/BiliConnectModal";
import UnfollowModal from "@/components/UnfollowModal";
import { Modal } from "antd";

const { Header, Sider, Content } = Layout;

export default function App() {
  const init = useStore((s) => s.init);
  const loading = useStore((s) => s.loading);
  const error = useStore((s) => s.error);
  const overview = useStore((s) => s.overview);
  const setImportOpen = useStore((s) => s.setImportOpen);
  const setFilter = useStore((s) => s.setFilter);
  const setConnectOpen = useStore((s) => s.setConnectOpen);
  const biliLoggedIn = useStore((s) => s.biliLoggedIn);
  const biliUname = useStore((s) => s.biliUname);
  const biliLogout = useStore((s) => s.biliLogout);
  const [modal, contextHolder] = Modal.useModal();

  useEffect(() => {
    void init();
  }, [init]);

  return (
    <Layout className="app-layout">
      <Header className="app-header">
        <div className="app-brand">
          <ClearOutlined /> FollowCleaner <span className="app-brand-sub">关注管家</span>
          <Tag color="default" style={{ marginLeft: 8 }}>
            v0.1
          </Tag>
        </div>
        <div className="app-stats">
          {overview && (
            <span className="muted">
              共 <b>{overview.total}</b> 个关注 · 未分类 <b>{overview.uncategorized}</b> 个
            </span>
          )}
        </div>
        <div className="app-actions">
          {biliLoggedIn ? (
            <Space size={4}>
              <Tag color="cyan" style={{ marginRight: 0 }}>
                B站：{biliUname ?? "已登录"}
              </Tag>
              <Tooltip title="断开B站登录（仅清除本机会话凭证）">
                <Button
                  size="small"
                  type="text"
                  icon={<LogoutOutlined />}
                  onClick={() => {
                    modal.confirm({
                      title: "断开哔哩哔哩账号连接？",
                      content: "仅清除本机保存的登录会话，不会对账号做任何操作。",
                      okText: "断开",
                      cancelText: "取消",
                      onOk: () => biliLogout(),
                    });
                  }}
                />
              </Tooltip>
            </Space>
          ) : (
            <Button icon={<ApiOutlined />} onClick={() => setConnectOpen(true)}>
              接入B站账号
            </Button>
          )}
          {biliLoggedIn && (
            <Button
              type="default"
              icon={<ApiOutlined />}
              onClick={() => setConnectOpen(true)}
            >
              同步关注
            </Button>
          )}
          <Button
            type="primary"
            icon={<ImportOutlined />}
            onClick={() => setImportOpen(true)}
          >
            导入关注列表
          </Button>
        </div>
      </Header>

      <Layout>
        <Sider className="app-sider" width={260} theme="light">
          <CategoryTree />
        </Sider>

        <Content className="app-content">
          {error && (
            <Alert
              type="error"
              showIcon
              style={{ marginBottom: 12 }}
              message="数据加载失败"
              description={
                <span>
                  {error}
                  <Button
                    size="small"
                    type="link"
                    onClick={() => {
                      setFilter({});
                      void init();
                    }}
                  >
                    重试
                  </Button>
                </span>
              }
            />
          )}
          <FilterBar />
          <Spin spinning={loading}>
            <AccountTable />
          </Spin>
          <BatchBar />
        </Content>
      </Layout>

      <DetailPanel />
      <ImportModal />
      <AssignCategoryModal />
      <BiliConnectModal />
      <UnfollowModal />
      {contextHolder}
    </Layout>
  );
}
