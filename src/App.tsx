import { useEffect } from "react";
import { Layout, Button, Alert, Spin, Tag } from "antd";
import { ImportOutlined, ClearOutlined } from "@ant-design/icons";
import { useStore } from "@/stores/data";
import CategoryTree from "@/components/CategoryTree";
import FilterBar from "@/components/FilterBar";
import AccountTable from "@/components/AccountTable";
import DetailPanel from "@/components/DetailPanel";
import BatchBar from "@/components/BatchBar";
import ImportModal from "@/components/ImportModal";
import AssignCategoryModal from "@/components/AssignCategoryModal";

const { Header, Sider, Content } = Layout;

export default function App() {
  const init = useStore((s) => s.init);
  const loading = useStore((s) => s.loading);
  const error = useStore((s) => s.error);
  const overview = useStore((s) => s.overview);
  const setImportOpen = useStore((s) => s.setImportOpen);
  const setFilter = useStore((s) => s.setFilter);

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
        <Button
          type="primary"
          icon={<ImportOutlined />}
          onClick={() => setImportOpen(true)}
        >
          导入关注列表
        </Button>
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
    </Layout>
  );
}
