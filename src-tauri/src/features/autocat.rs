// 自动分类：规则引擎一次性执行
// 规则：
//   verifyType 按账号认证类型
//   followAge  按关注时长（6个月内 / 6~12个月 / 1~3年 / 3年前）
//   smart      智能识别（内置兴趣词库，游戏细分到具体游戏，兜底「其他」）
//   keywords   自定义关键词（匹配昵称与简介）
// 产物：统一挂在根级父分类「自动分类」下；重跑幂等（INSERT OR IGNORE）
// 预留：规则持久化、同步后自动重跑、按平台限定范围 —— 后续版本
use crate::db::DbState;
use crate::error::{AppError, AppResult};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tauri::State;
use uuid::Uuid;

const PARENT_NAME: &str = "自动分类";
const DAY: i64 = 86_400;

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct KeywordRule {
    pub category_name: String,
    /// 多个关键词任一命中即归入该分类
    pub keywords: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoCategorizeArgs {
    /// verifyType | followAge | smart | keywords
    pub rule_type: String,
    pub keyword_rules: Option<Vec<KeywordRule>>,
    pub dry_run: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AutoCategorizeGroup {
    /// 分类路径（smart 规则的游戏细分为「游戏/原神」）
    pub category_name: String,
    pub count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoCategorizeResult {
    pub groups: Vec<AutoCategorizeGroup>,
    pub total_assigned: usize,
    pub dry_run: bool,
}

struct AccountRow {
    id: String,
    display_name: String,
    account_type: String,
    followed_at: Option<i64>,
    note: Option<String>,
}

fn type_label(t: &str) -> &str {
    match t {
        "brand" => "机构官方",
        "official" => "个人认证",
        "subscription_account" => "公众号",
        "video_account" => "视频号",
        "personal" => "普通账号",
        _ => "其他账号",
    }
}

/// 关注时长档位（与筛选器的 follow_age_buckets 对齐）
fn follow_age_label(ts: Option<i64>, now: i64) -> &'static str {
    let Some(ts) = ts.filter(|t| *t > 0) else {
        return "未知时间";
    };
    let days = (now - ts) / DAY;
    if days < 180 {
        "6个月内关注"
    } else if days < 365 {
        "6~12个月前关注"
    } else if days < 3 * 365 {
        "1~3年前关注"
    } else {
        "3年前关注"
    }
}

/// 智能分类内置词库：(分类路径, 关键词)。按优先级排列——
/// 具体游戏最先（最特异），宽泛的「游戏」兜底段最后，未命中归「其他」。
/// 匹配账号昵称 + 简介（sign），不区分大小写。
const SMART_DICT: &[(&[&str], &[&str])] = &[
    // ---- 具体游戏（二级分类：游戏/<名称>）----
    (&["游戏", "原神"], &["原神"]),
    (&["游戏", "明日方舟"], &["明日方舟", "方舟"]),
    (&["游戏", "崩坏"], &["崩坏", "星穹铁道", "星铁"]),
    (&["游戏", "王者荣耀"], &["王者荣耀", "王者"]),
    (&["游戏", "英雄联盟"], &["英雄联盟", "lol", "LOL"]),
    (&["游戏", "我的世界"], &["我的世界", "minecraft", "MC"]),
    (&["游戏", "CS"], &["csgo", "cs2", "cs go", "counter strike"]),
    (&["游戏", "绝地求生"], &["绝地求生", "pubg", "吃鸡"]),
    (&["游戏", "无畏契约"], &["无畏契约", "valorant", "瓦罗兰特"]),
    (&["游戏", "第五人格"], &["第五人格"]),
    (&["游戏", "阴阳师"], &["阴阳师"]),
    (&["游戏", "任天堂"], &["任天堂", "塞尔达", " Switch", "switch"]),
    (&["游戏", "蛋仔派对"], &["蛋仔"]),
    (&["游戏", "光遇"], &["光遇", "sky光遇"]),
    (&["游戏", "主机游戏"], &["ps5", "ps4", "xbox", "主机"]),
    // ---- 兴趣大类 ----
    (&["编程"], &["编程", "程序员", "代码", "开发", "前端", "后端", "python", "java", "javascript", "算法", "人工智能", "机器学习", "深度学习", "linux", "软件", "计算机", "科技", "数码"]),
    (&["手工"], &["手工", "diy", "DIY", "木工", "编织", "折纸", "黏土", "手作"]),
    (&["绘画"], &["绘画", "画师", "插画", "原画", "素描", "水彩", "板绘", "画画", "美术", "漫画"]),
    (&["美食"], &["美食", "做饭", "烹饪", "料理", "探店", "菜谱", "烘焙", "厨房", "吃货", "干饭"]),
    (&["ASMR"], &["asmr", "ASMR", "助眠", "耳语", "触发音", "哄睡"]),
    (&["音乐"], &["音乐", "翻唱", "钢琴", "吉他", "小提琴", "作曲", "编曲", "歌手", "民谣", "说唱", "rap", "电音", "乐队", "BGM"]),
    (&["舞蹈"], &["舞蹈", "宅舞", "跳舞", "编舞"]),
    (&["动漫"], &["动漫", "动画", "番剧", "二次元", "声优", "cos", "cosplay", "国创", "日漫", "手办"]),
    (&["搞笑"], &["搞笑", "沙雕", "鬼畜", "整活", "段子"]),
    (&["知识"], &["知识", "科普", "历史", "心理", "法律", "医学", "教育", "数学", "物理", "化学", "英语", "日语", "学习", "考研", "读书", "书籍"]),
    (&["生活"], &["生活", "vlog", "VLOG", "日常", "旅行", "露营", "家居", "收纳", "田园", "搬家", "装修"]),
    (&["美妆"], &["美妆", "化妆", "护肤", "穿搭", "时尚", "口红"]),
    (&["运动"], &["健身", "运动", "篮球", "足球", "羽毛球", "跑步", "瑜伽", "减肥", "滑板", "乒乓", "网球"]),
    (&["影视"], &["电影", "影视", "剧集", "电视剧", "纪录片", "影评", "预告片"]),
    (&["萌宠"], &["猫咪", "猫猫", "小猫", "橘猫", "布偶猫", "狗狗", "小狗", "柯基", "哈士奇", "宠物", "萌宠", "动物", "动物园"]),
    (&["汽车"], &["汽车", "机车", "摩托车", "改装车", "赛车", "飙车"]),
    (&["财经"], &["财经", "理财", "股票", "基金", "经济学", "投资", "金融"]),
    // ---- 宽泛游戏兜底（放在大类之后，避免抢走更特异的匹配）----
    (&["游戏"], &["游戏", "实况", "攻略", "主播", "解说", "电竞", "手游", "端游", "网游"]),
];

/// 读取当前在关注中的账号（已取关的不参与分类）
fn load_accounts(conn: &Connection) -> AppResult<Vec<AccountRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, display_name, account_type, followed_at, note
         FROM accounts WHERE status = 'following'",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(AccountRow {
                id: r.get(0)?,
                display_name: r.get(1)?,
                account_type: r.get(2)?,
                followed_at: r.get(3)?,
                note: r.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 智能分类：按词库优先级取首个命中；未命中归「其他」
fn smart_classify(accounts: &[AccountRow]) -> BTreeMap<String, Vec<String>> {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for a in accounts {
        let name = a.display_name.to_lowercase();
        let note = a.note.as_deref().unwrap_or("").to_lowercase();
        let matched = SMART_DICT.iter().find(|(_, kws)| {
            kws.iter()
                .any(|k| name.contains(k.to_lowercase().as_str()) || note.contains(k.to_lowercase().as_str()))
        });
        let label = match matched {
            Some((path, _)) => path.join("/"),
            None => "其他".to_string(),
        };
        groups.entry(label).or_default().push(a.id.clone());
    }
    groups
}

fn build_groups(rule: &AutoCategorizeArgs, accounts: &[AccountRow]) -> AppResult<BTreeMap<String, Vec<String>>> {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    match rule.rule_type.as_str() {
        "verifyType" => {
            for a in accounts {
                groups
                    .entry(type_label(&a.account_type).to_string())
                    .or_default()
                    .push(a.id.clone());
            }
        }
        "followAge" => {
            let now = Utc::now().timestamp();
            for a in accounts {
                groups
                    .entry(follow_age_label(a.followed_at, now).to_string())
                    .or_default()
                    .push(a.id.clone());
            }
        }
        "smart" => {
            groups = smart_classify(accounts);
        }
        "keywords" => {
            let rules = rule
                .keyword_rules
                .as_ref()
                .filter(|rs| !rs.is_empty())
                .ok_or_else(|| AppError::Validation("请至少填写一条关键词规则".into()))?;
            for r in rules {
                if r.category_name.trim().is_empty() {
                    return Err(AppError::Validation("关键词规则的分类名不能为空".into()));
                }
                if r.keywords.iter().all(|k| k.trim().is_empty()) {
                    return Err(AppError::Validation(format!(
                        "分类「{}」至少需要一个关键词",
                        r.category_name
                    )));
                }
            }
            for r in rules {
                let kws: Vec<String> = r
                    .keywords
                    .iter()
                    .map(|k| k.trim().to_lowercase())
                    .filter(|k| !k.is_empty())
                    .collect();
                for a in accounts {
                    let name = a.display_name.to_lowercase();
                    let note = a.note.as_deref().unwrap_or("").to_lowercase();
                    if kws.iter().any(|k| name.contains(k.as_str()) || note.contains(k.as_str())) {
                        groups
                            .entry(r.category_name.trim().to_string())
                            .or_default()
                            .push(a.id.clone());
                    }
                }
            }
        }
        other => {
            return Err(AppError::Validation(format!("未知规则类型: {other}")));
        }
    }
    groups.retain(|_, ids| !ids.is_empty());
    Ok(groups)
}

/// 按名称找分类（同父下），没有则创建；返回分类 id
fn ensure_category(conn: &Connection, name: &str, parent_id: Option<&str>) -> AppResult<String> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM categories WHERE name = ?1 AND parent_id IS ?2 LIMIT 1",
            params![name, parent_id],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = Uuid::new_v4().to_string();
    let ts = Utc::now().timestamp();
    let next_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM categories WHERE parent_id IS ?1",
        params![parent_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO categories (id, name, parent_id, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![id, name, parent_id, next_order, ts],
    )?;
    Ok(id)
}

/// 按路径逐级创建/复用分类（如 ["游戏", "原神"]），返回末级分类 id
fn ensure_category_path(conn: &Connection, path: &[String]) -> AppResult<String> {
    let mut parent: Option<String> = None;
    let mut last = String::new();
    for seg in path {
        last = ensure_category(conn, seg, parent.as_deref())?;
        parent = Some(last.clone());
    }
    Ok(last)
}

#[tauri::command]
pub async fn auto_categorize(
    state: State<'_, DbState>,
    args: AutoCategorizeArgs,
) -> AppResult<AutoCategorizeResult> {
    let mut conn = state.0.lock().unwrap();
    let accounts = load_accounts(&conn)?;
    if accounts.is_empty() {
        return Err(AppError::Validation("当前没有可分类的在关注账号，请先导入或同步".into()));
    }
    let groups = build_groups(&args, &accounts)?;
    if groups.is_empty() {
        return Err(AppError::Validation(
            "没有账号命中规则，请调整规则后重试（关键词规则不区分大小写、匹配昵称与简介）".into(),
        ));
    }

    if args.dry_run {
        return Ok(AutoCategorizeResult {
            groups: groups
                .iter()
                .map(|(name, ids)| AutoCategorizeGroup {
                    category_name: name.clone(),
                    count: ids.len(),
                })
                .collect(),
            total_assigned: 0,
            dry_run: true,
        });
    }

    let tx = conn.transaction()?;
    let parent_id = ensure_category(&tx, PARENT_NAME, None)?;
    let mut total_assigned = 0usize;
    let mut result_groups = Vec::new();
    for (name, ids) in &groups {
        // 路径按 '/' 拆分（smart 规则的「游戏/原神」 -> 两级分类）
        let path: Vec<String> = name
            .split('/')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        let cid = if path.len() > 1 {
            ensure_category_path(&tx, &path)?
        } else {
            ensure_category(&tx, &path[0], Some(&parent_id))?
        };
        for aid in ids {
            let n = tx.execute(
                "INSERT OR IGNORE INTO account_categories (account_id, category_id)
                 VALUES (?1, ?2)",
                params![aid, cid],
            )?;
            total_assigned += n;
        }
        result_groups.push(AutoCategorizeGroup {
            category_name: name.clone(),
            count: ids.len(),
        });
    }
    tx.commit()?;

    log::info!(
        "auto_categorize rule={} -> {} groups, {} new assignments",
        args.rule_type,
        result_groups.len(),
        total_assigned
    );
    Ok(AutoCategorizeResult {
        groups: result_groups,
        total_assigned,
        dry_run: false,
    })
}
