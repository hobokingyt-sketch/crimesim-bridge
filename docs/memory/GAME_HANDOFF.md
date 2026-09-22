# Future managed game memory

This is a **design brief**, not an implemented game and not Bridge runtime state.

The game is a top-down criminal-life simulation. The player is one physical simulated character,
starting broke with low-level criminal professions under Hustles. Administrative menus issue orders;
there is no direct character movement. Travel, carrying goods/cash, dropping items off, and mandatory
sleep consume world time. Pause and 1/2/3 speed controls exist in the design. Stress, health and injuries
are planned; food, shelter and appearance micromanagement are excluded. Profession levels run 1-100,
with useful long-term value and larger milestones. Motion is the initial unlock value; deeper Rep is later.
Recruits begin as Associates. Player-allocated Respect and simulated NPC respect are separate.

UI reference is 2560x1440: the map dominates, top holds globals/time/activity, left holds compact map
utilities, right holds clickable alert icons, bottom holds root sections/subsections and contextual controls.
A normal bottom stack occupies about 25-35% total height; single-selection inspection coexists with the
open management context. No rendered interiors initially; later important buildings may have deeper data.
Names use grounded street/organized-crime language, not generic RPG attributes or invented slang.
Rival/vehicle details and the first profession's exact name are not settled by this document.

When PACK-1 is implemented, generate this game's own AGENTS.md, current implementation checkpoint,
module registry and simulation/UI contracts in its source workspace. Do not copy Bridge's engine worker
or installer backlog into the game. Planned features must not be shown as completed mechanics.
