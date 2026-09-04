import { useMemo, useState } from "react";
import { Modal, Tree, Empty } from "antd";
import type { TreeDataNode } from "antd";
import { useStore } from "@/stores/data";

export default function AssignCategoryModal() {
  const open = useStore((s) => s.assignOpen);
  const setOpen = useStore((s) => s.setAssignOpen);
  const categories = useStore((s) => s.categories);
  const selectedIds = useStore((s) => s.selectedIds);
  const assignCategories = useStore((s) => s.assignCategories);

  const [checked, setChecked] = useState<string[]>([]);

  const treeData = useMemo<TreeDataNode[]>(() => {
    const byParent = new Map<string | null, typeof categories>();
    for (const c of categories) {
      const key = c.parentId ?? null;
      if (!byParent.has(key)) byParent.set(key, []);
      byParent.get(key)!.push(c);
    }
    for (const list of byParent.values()) {
      list.sort((a, b) => a.sortOrder - b.sortOrder);
    }
    const build = (parentId: string | null): TreeDataNode[] =>
      (byParent.get(parentId) ?? []).map((c) => ({
        key: c.id,
        title: c.name,
        children: build(c.id),
      }));
    return build(null);
  }, [categories]);

  const onOk = async () => {
    if (checked.length === 0) return;
    await assignCategories(checked);
    setChecked([]);
  };

  return (
    <Modal
      title={`分配分类（${selectedIds.length} 个账号）`}
      open={open}
      onOk={onOk}
      onCancel={() => setOpen(false)}
      okText="确认分配"
      cancelText="取消"
      okButtonProps={{ disabled: checked.length === 0 }}
      afterClose={() => setChecked([])}
    >
      <p className="muted" style={{ fontSize: 12 }}>
        勾选目标分类（支持多选，追加到账号已有分类，不覆盖）。
      </p>
      {treeData.length === 0 ? (
        <Empty description="还没有分类，请先在左侧创建" />
      ) : (
        <Tree
          checkable
          checkStrictly
          defaultExpandAll
          treeData={treeData}
          checkedKeys={checked}
          onCheck={(keys) => {
            // checkStrictly 模式下 keys 为 { checked, halfChecked }
            if (Array.isArray(keys)) setChecked(keys as string[]);
            else setChecked(keys.checked as string[]);
          }}
        />
      )}
    </Modal>
  );
}
