// 用户模块监控 dashboard
// 渲染:sh generate.sh(输出到 ../user.json)
local g = import 'g.libsonnet';
local p = import 'lib/panels.libsonnet';
local ops = import 'lib/ops.libsonnet';

// ---- 布局参数(与 dashboard-design-spec.md 一致)----
local httpRowY = 0;      // HTTP 汇总行:row y=0,面板 y=1
local httpPanelY = 1;
local opStartY = 2;      // 第一个操作 row 的 y(行间隔 9 = 面板高 8 + row 高 1)
local opStepY = 9;

// user 的操作 id 间隔为 5(历史遗留:早期每操作含 4 个面板槽,
// 与 auth 的 4 不同,保留现状避免重新编号)
local idStep = 5;

// ---- 面板组装 ----

local httpPanels = [
  p.row('HTTP 请求 (http)', 2001, httpRowY),
  p.httpQpsPanel('user', 2002, 0, httpPanelY),
  p.httpErrorRatePanel('user', 2003, 8, httpPanelY),
  p.httpLatencyPanel('user', 2004, 16, httpPanelY),
];

local opBlocks = std.foldl(
  function(acc, op)
    local idx = acc.idx;
    local y = opStartY + idx * opStepY;
    local idBase = idx * idStep + 1;
    acc + {
      idx: idx + 1,
      panels: acc.panels + [
        p.row(op.rowTitle, idBase, y),
        p.durationPanel(ops.user.crate, op.name, op.func, idBase + 1, 0, y + 1),
        p.substepsPanel(ops.user.crate, op.name, op.func, op.steps, idBase + 2, 6, y + 1),
        p.qpsSuccessPanel(ops.user.crate, op.name, op.func, idBase + 3, 12, y + 1),
      ],
    },
  ops.user.ops,
  { idx: 0, panels: [] },
).panels;

// ---- dashboard 装配 ----

local dashboard =
  g.dashboard.new('用户模块监控')
  + g.dashboard.withUid('user-module-monitoring')
  + g.dashboard.withTags(['user', 'api', 'monitoring'])
  + g.dashboard.withEditable(true)
  + g.dashboard.withFiscalYearStartMonth(0)
  + g.dashboard.withLinks([])
  + g.dashboard.withRefresh('5s')
  + g.dashboard.withSchemaVersion(39)
  + g.dashboard.withTemplating({ list: [] })
  + g.dashboard.withTimezone('')
  + g.dashboard.time.withFrom('now-5m')
  + g.dashboard.time.withTo('now')
  + g.dashboard.withPanels(httpPanels + opBlocks, setPanelIDs=false)
  + {
    // 以下字段 grafonnet 未提供 mixin,直接扩展;version 由 Grafana 管理,不在此生成
    graphTooltip: 0,
    timepicker: {},
    annotations: {
      list: [
        {
          builtIn: 1,
          datasource: { type: 'grafana', uid: '-- Grafana --' },
          enable: true,
          hide: true,
          iconColor: 'rgba(0, 211, 255, 1)',
          name: 'Annotations & Alerts',
          type: 'dashboard',
        },
      ],
    },
  };

dashboard