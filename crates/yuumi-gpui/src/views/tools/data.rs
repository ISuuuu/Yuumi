//! 工具箱常数与静态数据模型定义
//!
//! 包含常用英雄、常用召唤师技能、游戏模式、段位选项、好友清理筛选与装配勋章等。

/// 常用英雄条目定义
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChampionEntry {
    pub id: i32,
    pub name: &'static str,
    pub title: &'static str,
    pub pinyin: &'static str,
}

/// 常用英雄列表定义（支持按拼音、中文名、称号、ID 过滤）
pub const CHAMPIONS: &[ChampionEntry] = &[
    ChampionEntry {
        id: 157,
        name: "亚索",
        title: "疾风剑豪",
        pinyin: "yasuo",
    },
    ChampionEntry {
        id: 777,
        name: "永恩",
        title: "封魔剑魂",
        pinyin: "yongen",
    },
    ChampionEntry {
        id: 64,
        name: "李青",
        title: "盲僧",
        pinyin: "mangsen",
    },
    ChampionEntry {
        id: 145,
        name: "卡莎",
        title: "虚空之女",
        pinyin: "kasha",
    },
    ChampionEntry {
        id: 222,
        name: "金克丝",
        title: "暴走萝莉",
        pinyin: "jinkesi",
    },
    ChampionEntry {
        id: 81,
        name: "伊泽瑞尔",
        title: "探险家",
        pinyin: "ez",
    },
    ChampionEntry {
        id: 99,
        name: "拉克丝",
        title: "光辉女郎",
        pinyin: "lakesi",
    },
    ChampionEntry {
        id: 238,
        name: "劫",
        title: "影流之主",
        pinyin: "jie",
    },
    ChampionEntry {
        id: 17,
        name: "提莫",
        title: "迅捷斥候",
        pinyin: "timo",
    },
    ChampionEntry {
        id: 67,
        name: "薇恩",
        title: "暗夜猎手",
        pinyin: "weien",
    },
    ChampionEntry {
        id: 11,
        name: "易",
        title: "无极剑圣",
        pinyin: "jiansheng",
    },
    ChampionEntry {
        id: 122,
        name: "德莱厄斯",
        title: "诺克萨斯之手",
        pinyin: "nuoshou",
    },
    ChampionEntry {
        id: 86,
        name: "盖伦",
        title: "德玛西亚之力",
        pinyin: "gailun",
    },
    ChampionEntry {
        id: 103,
        name: "阿狸",
        title: "九尾妖狐",
        pinyin: "ali",
    },
    ChampionEntry {
        id: 412,
        name: "锤石",
        title: "魂锁典狱长",
        pinyin: "chuishi",
    },
    ChampionEntry {
        id: 555,
        name: "派克",
        title: "血港鬼影",
        pinyin: "paike",
    },
    ChampionEntry {
        id: 53,
        name: "布里茨",
        title: "蒸汽机器人",
        pinyin: "jiqiren",
    },
    ChampionEntry {
        id: 39,
        name: "艾瑞莉娅",
        title: "刀锋舞者",
        pinyin: "daomei",
    },
    ChampionEntry {
        id: 92,
        name: "锐雯",
        title: "放逐之刃",
        pinyin: "ruiwen",
    },
    ChampionEntry {
        id: 517,
        name: "塞拉斯",
        title: "解脱者",
        pinyin: "sailasi",
    },
    ChampionEntry {
        id: 234,
        name: "佛耶戈",
        title: "破败之王",
        pinyin: "foyego",
    },
    ChampionEntry {
        id: 350,
        name: "悠米",
        title: "魔法猫咪",
        pinyin: "youmi",
    },
    ChampionEntry {
        id: 51,
        name: "凯特琳",
        title: "皮城女警",
        pinyin: "nvjing",
    },
    ChampionEntry {
        id: 22,
        name: "艾希",
        title: "寒冰射手",
        pinyin: "hanbing",
    },
    ChampionEntry {
        id: 89,
        name: "蕾欧娜",
        title: "曙光女神",
        pinyin: "rinu",
    },
    ChampionEntry {
        id: 25,
        name: "莫甘娜",
        title: "堕落天使",
        pinyin: "moganna",
    },
    ChampionEntry {
        id: 55,
        name: "卡特琳娜",
        title: "不祥之刃",
        pinyin: "katelinna",
    },
    ChampionEntry {
        id: 104,
        name: "格雷福斯",
        title: "法外狂徒",
        pinyin: "fawaikuangtu",
    },
    ChampionEntry {
        id: 141,
        name: "凯隐",
        title: "影流之镰",
        pinyin: "kaiyin",
    },
    ChampionEntry {
        id: 32,
        name: "阿木木",
        title: "殇之木乃伊",
        pinyin: "amumu",
    },
    ChampionEntry {
        id: 54,
        name: "墨菲特",
        title: "熔岩巨兽",
        pinyin: "shitouren",
    },
    ChampionEntry {
        id: 63,
        name: "布兰德",
        title: "复仇焰魂",
        pinyin: "huonan",
    },
    ChampionEntry {
        id: 24,
        name: "贾克斯",
        title: "武器大师",
        pinyin: "wuqidashi",
    },
    ChampionEntry {
        id: 58,
        name: "雷克顿",
        title: "荒漠屠夫",
        pinyin: "eyu",
    },
    ChampionEntry {
        id: 84,
        name: "阿卡丽",
        title: "离群之刺",
        pinyin: "akali",
    },
    ChampionEntry {
        id: 202,
        name: "烬",
        title: "戏命师",
        pinyin: "jin",
    },
    ChampionEntry {
        id: 236,
        name: "卢锡安",
        title: "圣枪游侠",
        pinyin: "luxian",
    },
    ChampionEntry {
        id: 147,
        name: "赛娜",
        title: "涤魂圣枪",
        pinyin: "saina",
    },
    ChampionEntry {
        id: 498,
        name: "霞",
        title: "逆羽",
        pinyin: "xia",
    },
    ChampionEntry {
        id: 497,
        name: "洛",
        title: "幻翎",
        pinyin: "luo",
    },
    ChampionEntry {
        id: 875,
        name: "瑟提",
        title: "腕豪",
        pinyin: "wanhao",
    },
    ChampionEntry {
        id: 266,
        name: "亚托克斯",
        title: "暗裔剑魔",
        pinyin: "jianmo",
    },
    ChampionEntry {
        id: 114,
        name: "菲奥娜",
        title: "无双剑姬",
        pinyin: "jianji",
    },
    ChampionEntry {
        id: 150,
        name: "纳尔",
        title: "迷失之牙",
        pinyin: "naer",
    },
    ChampionEntry {
        id: 711,
        name: "薇古丝",
        title: "愁云使者",
        pinyin: "weigusi",
    },
    ChampionEntry {
        id: 142,
        name: "佐伊",
        title: "暮光星灵",
        pinyin: "zuoyi",
    },
    ChampionEntry {
        id: 268,
        name: "阿兹尔",
        title: "沙漠皇帝",
        pinyin: "shahuang",
    },
    ChampionEntry {
        id: 61,
        name: "奥莉安娜",
        title: "发条魔灵",
        pinyin: "fatiao",
    },
    ChampionEntry {
        id: 134,
        name: "辛德拉",
        title: "暗黑元首",
        pinyin: "xindela",
    },
    ChampionEntry {
        id: 126,
        name: "杰斯",
        title: "未来守护者",
        pinyin: "jiesi",
    },
];

/// 根据 ID 获取英雄名称
pub fn get_champion_name(id: i32) -> &'static str {
    CHAMPIONS
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.name)
        .unwrap_or("未知英雄")
}

/// 常用召唤师技能列表定义 (技能 ID, 中文名)
pub const SUMMONER_SPELLS: [(i32, &str); 9] = [
    (4, "闪现"),
    (14, "引燃"),
    (7, "治疗"),
    (11, "惩戒"),
    (12, "传送"),
    (21, "屏障"),
    (3, "虚弱"),
    (1, "净化"),
    (6, "疾跑"),
];

/// 根据 ID 获取召唤师技能名称
pub fn get_spell_name(id: i32) -> &'static str {
    SUMMONER_SPELLS
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, n)| *n)
        .unwrap_or("未知技能")
}

/// 常用游戏模式选项定义
pub const GAME_MODES: [(u32, &str); 11] = [
    (2400, "海克斯大乱斗"),
    (2450, "经典海斗"),
    (450, "极地大乱斗"),
    (430, "匹配模式 (自选)"),
    (420, "单双排位"),
    (440, "灵活排位"),
    (900, "无限火力"),
    (1020, "克隆模式"),
    (1300, "极限闪击"),
    (1700, "斗魂竞技场"),
    (4300, "经典模式"),
];

/// 下拉模式选项
pub const GAME_MODE_OPTIONS: [(&str, &str); 11] = [
    ("2400", "海克斯大乱斗"),
    ("2450", "经典海斗"),
    ("450", "极地大乱斗"),
    ("430", "匹配模式 (自选)"),
    ("420", "单双排位"),
    ("440", "灵活排位"),
    ("900", "无限火力"),
    ("1020", "克隆模式"),
    ("1300", "极限闪击"),
    ("1700", "斗魂竞技场"),
    ("4300", "经典模式"),
];

/// 段位伪装：队列选项
pub const SPOOF_QUEUES: [(&str, &str); 3] = [
    ("RANKED_SOLO_5x5", "单双排位"),
    ("RANKED_FLEX_SR", "灵活排位"),
    ("RANKED_TFT", "云顶之弈"),
];

/// 段位伪装：大段位选项
pub const SPOOF_TIERS: [(&str, &str); 11] = [
    ("UNRANKED", "未定级"),
    ("CHALLENGER", "最强王者"),
    ("GRANDMASTER", "傲世宗师"),
    ("MASTER", "超凡大师"),
    ("DIAMOND", "璀璨钻石"),
    ("EMERALD", "流光翡翠"),
    ("PLATINUM", "华贵铂金"),
    ("GOLD", "荣耀黄金"),
    ("SILVER", "不屈白银"),
    ("BRONZE", "英勇黄铜"),
    ("IRON", "坚韧黑铁"),
];

/// 段位伪装：小段位选项
pub const SPOOF_DIVISIONS: [&str; 4] = ["I", "II", "III", "IV"];

/// 模拟挑战勋章定义（用于自定义装配弹窗）
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChallengeBadge {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub tier: &'static str,
}

pub const MOCK_BADGES: &[ChallengeBadge] = &[
    ChallengeBadge {
        id: "b1",
        name: "绝代双骄",
        desc: "在排位中取得双排 10 连胜",
        tier: "apex",
    },
    ChallengeBadge {
        id: "b2",
        name: "神之眼",
        desc: "视野得分单局超过 100",
        tier: "high",
    },
    ChallengeBadge {
        id: "b3",
        name: "不灭传说",
        desc: "在一局比赛中取得超神且零阵亡",
        tier: "apex",
    },
    ChallengeBadge {
        id: "b4",
        name: "极地探险家",
        desc: "大乱斗胜场达到 500 场",
        tier: "mid",
    },
    ChallengeBadge {
        id: "b5",
        name: "纳什男爵克星",
        desc: "抢夺大龙累计超过 20 次",
        tier: "high",
    },
    ChallengeBadge {
        id: "b6",
        name: "新兵报到",
        desc: "完成新手教学与首胜",
        tier: "low",
    },
];

/// 模拟称号定义
pub const MOCK_TITLES: [(&str, &str); 4] = [
    ("none", "未佩戴称号"),
    ("t1", "无可匹敌"),
    ("t2", "极地霸王"),
    ("t3", "傲视群雄"),
];
