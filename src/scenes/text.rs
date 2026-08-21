//! UI 文案常量表：M1 先硬编码英文占位，集中于此；
//! M2 换 12 号域的 `Messages` 资源（链式回退 i18n），勿在此自造翻译机制。

/// `TitleScene` 进入游戏按钮（SPD `TitleScene` 文案键 `enter`）
pub const BTN_ENTER_DUNGEON: &str = "Enter the Dungeon";

/// `TitleScene` 退出按钮
pub const BTN_QUIT: &str = "Quit";

/// `InGame` HUD 前缀（M2 调试渲染阶段；正式 HUD 是 M3+ 范围）
pub const IN_GAME_HUD_PREFIX: &str = "Esc: back to title  |";

/// HUD 英雄生命标签（M4 调试 HUD；正式状态栏是 M5+ 范围）
pub const HUD_HP_LABEL: &str = "HP";

/// HUD 英雄等级标签
pub const HUD_LVL_LABEL: &str = "Lv";

/// HUD 英雄经验标签
pub const HUD_EXP_LABEL: &str = "EXP";
