import { useMemo, useState } from "react";
import { Button, Input, Modal, Tree, Dropdown, type TreeDataNode } from "antd";
import {
  FolderAddOutlined,
  FolderOutlined,
  AppstoreOutlined,
  QuestionOutlined,
  ImportOutlined,
  MoreOutlined,
} from "@ant-design/icons";
import type { Category } from "@/types";
import { ALL_ID, UNCATEGORIZED_ID, useStore } from "@/stores/data";

const VIRTUAL_KEYS = new Set([ALL_ID, UNCATEGORIZED_ID]);

interface CatNode extends TreeDataNode {
  key: string;
  title: React.ReactNode;
  children?: CatNode[];
  isVirtual?: boolean;
}

export default function CategoryTree() {
  const categories = useStore((s) => s.categories);
  const counts = useStore((s) => s.counts);
  const overview = useStore((s) => s.overview);
  const categoryId = useStore((s) => s.categoryId);
  const setFilter = useStore((s) => s.setFilter);
  const createCategory = useStore((s) => s.createCategory);
  const renameCategory = useStore((s) => s.renameCategory);
  const deleteCategory = useStore((s) => s.deleteCategory);
  const moveCategory = useStore((s) => s.moveCategory);
  const setImportOpen = useStore((s) => s.setImportOpen);

  const [modal, contextHolder] = Modal.useModal();

  // 新建/重命名弹窗状态
  const [editing, setEditing] = useState<{
    mode: "create-root" | "create-child" | "rename";
    id?: string;
    parentId?: string | null;
    defaultName?: string;
  } | null>(null);
  const [name, setName] = useState("");

  const treeData = useMemo<CatNode[]>(() => {
    const byParent = new Map<string | null, Category[]>();
    for (const c of categories) {
      const key = c.parentId ?? null;
      if (!byParent.has(key)) byParent.set(key, []);
      byParent.get(key)!.push(c);
    }
    for (const list of byParent.values()) {
      list.sort((a, b) => a.sortOrder - b.sortOrder);
    }
    const build = (parentId: string | null): CatNode[] => {
      const list = byParent.get(parentId) ?? [];
      return list.map((c) => ({
        key: c.id,
        title: renderTitle(c),
        children: build(c.id),
        isLeaf: (byParent.get(c.id)?.length ?? 0) === 0,
      }));
    };
    const virtual: CatNode[] = [
      {
        key: ALL_ID,
        title: (
          <span>
            <AppstoreOutlined /> 全部账号{" "}
            <b style={{ color: "#8c8c8c" }}>{overview?.total ?? 0}</b>
          </span>
        ),
        isVirtual: true,
      },
      {
        key: UNCATEGORIZED_ID,
        title: (
          <span>
            <QuestionOutlined /> 未分类{" "}
            <b style={{ color: "#8c8c8c" }}>{overview?.uncategorized ?? 0}</b>
          </span>
        ),
        isVirtual: true,
      },
    ];
    return [...virtual, ...build(null)];
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [categories, counts, overview]);

  function renderTitle(c: Category) {
    const n = counts[c.id] ?? 0;
    const menuItems = [
      {
        key: "child",
        label: "新建子分类",
        onClick: () => {
          setEditing({ mode: "create-child", parentId: c.id });
          setName("");
        },
      },
      {
        key: "rename",
        label: "重命名",
        onClick: () => {
          setEditing({ mode: "rename", id: c.id, defaultName: c.name });
          setName(c.name);
        },
      },
      {
        key: "delete",
        label: <span style={{ color: "#ff4d4f" }}>删除</span>,
        onClick: () => {
          modal.confirm({
            title: `删除分类「${c.name}」？`,
            content:
              "该分类下的账号不会被删除，将移入「未分类」。若含子分类请先删除子分类。",
            okText: "删除",
            okButtonProps: { danger: true },
            cancelText: "取消",
            onOk: () => deleteCategory(c.id),
          });
        },
      },
    ];
    return (
      <span className="cat-node">
        <FolderOutlined /> <span className="cat-name">{c.name}</span>{" "}
        <span className="cat-count">{n}</span>
        <Dropdown menu={{ items: menuItems }} trigger={["click"]}>
          <span className="cat-more" onClick={(e) => e.stopPropagation()}>
            <MoreOutlined />
          </span>
        </Dropdown>
      </span>
    );
  }

  const submitEdit = async () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    try {
      if (editing?.mode === "create-root") await createCategory(trimmed, null);
      if (editing?.mode === "create-child")
        await createCategory(trimmed, editing.parentId ?? null);
      if (editing?.mode === "rename" && editing.id)
        await renameCategory(editing.id, trimmed);
      setEditing(null);
    } catch (e) {
      modal.error({ title: "操作失败", content: (e as Error).message });
    }
  };

  return (
    <div className="sider-cats">
      <div className="sider-header">
        <span className="sider-title">分类</span>
        <span>
          <Button
            size="small"
            type="text"
            icon={<FolderAddOutlined />}
            onClick={() => {
              setEditing({ mode: "create-root" });
              setName("");
            }}
          />
          <Button
            size="small"
            type="text"
            icon={<ImportOutlined />}
            onClick={() => setImportOpen(true)}
          />
        </span>
      </div>

      <Tree
        blockNode
        showLine
        draggable={(node) => !VIRTUAL_KEYS.has(node.key as string)}
        allowDrop={(info) => {
          // 虚拟节点不能作为拖入目标；不允许拖到自己后代（Rust 端也有校验）
          if (VIRTUAL_KEYS.has(info.dropNode.key as string)) return false;
          return true;
        }}
        treeData={treeData}
        selectedKeys={[categoryId]}
        onSelect={(keys) => {
          const k = keys[0] as string;
          if (k) setFilter({ categoryId: k });
        }}
        onDrop={async (info) => {
          const dragKey = info.dragNode.key as string;
          if (VIRTUAL_KEYS.has(dragKey)) return;
          const dropKey = info.node.key as string;
          const dropKeyPos = info.node.pos.split("-");
          const dropPosition =
            info.dropPosition - Number(dropKeyPos[dropKeyPos.length - 1]);

          try {
            if (info.dropToGap) {
              // 放到目标同级
              const target = categories.find((c) => c.id === dropKey);
              const newParentId = target?.parentId ?? null;
              const siblings = categories
                .filter((c) => (c.parentId ?? null) === newParentId)
                .sort((a, b) => a.sortOrder - b.sortOrder);
              const idx = siblings.findIndex((c) => c.id === dropKey);
              const position = dropPosition === -1 ? idx : idx + 1;
              await moveCategory(dragKey, newParentId, position);
            } else {
              // 放到目标内部（成为子节点，插到最前）
              if (VIRTUAL_KEYS.has(dropKey)) return;
              await moveCategory(dragKey, dropKey, 0);
            }
          } catch (e) {
            modal.error({ title: "移动分类失败", content: (e as Error).message });
          }
        }}
      />

      <Modal
        title={
          editing?.mode === "rename"
            ? "重命名分类"
            : editing?.mode === "create-child"
              ? "新建子分类"
              : "新建分类"
        }
        open={!!editing}
        onOk={submitEdit}
        onCancel={() => setEditing(null)}
        okText="确定"
        cancelText="取消"
      >
        <Input
          autoFocus
          placeholder="分类名称"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onPressEnter={submitEdit}
          maxLength={30}
        />
      </Modal>
      {contextHolder}
    </div>
  );
}
