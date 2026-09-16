// 多级缓存监控 dashboard
// 渲染:jsonnet -J vendor cache.jsonnet | jq -S 'del(.panels[].pluginVersion)' > ../cache.json
local g = import 'g.libsonnet';
local p = import 'lib/panels.libsonnet';

// ---- 布局参数(行间隔 9 = 面板高 8 + row 高 1)----
local hitRowY = 0;      // 命中率行:面板 y=1
local durationRowY = 9; // 耗时行:面板 y=10
local capacityRowY = 18;// 容量与穿透行:面板 y=19

local ts = g.panel.timeSeries;

local DS_UID = 'Prometheus';

// 命中率:0-100 百分比,<90 红 / 90-99 黄 / >=99 绿
local hitRateThresholds =
  ts.standardOptions.thresholds.withMode('absolute')
  + ts.standardOptions.thresholds.withSteps([
      { color: 'red', value: null },
      { color: 'yellow', value: 90 },
      { color: 'green', value: 99 },
    ])
  + ts.standardOptions.withMin(0)
  + ts.standardOptions.withMax(100);

// 可见图例(底部)+ lastNotNull 统计,与现状一致
local legend =
  ts.options.legend.withCalcs(['lastNotNull'])
  + ts.options.legend.withDisplayMode('list')
  + ts.options.legend.withPlacement('bottom')
  + ts.options.tooltip.withMode('single')
  + ts.options.tooltip.withSort('none');

// 单查询 timeseries 面板
local seriesPanel(title, id, x, y, w, unit, expr, legendFormat, thresholds) =
  ts.new(title)
  + ts.queryOptions.withDatasource('prometheus', DS_UID)
  + ts.queryOptions.withTargets([p.promTarget(expr, legendFormat, 'A')])
  + p.baseCustom()
  + legend
  + ts.standardOptions.withUnit(unit)
  + thresholds
  + ts.panelOptions.withGridPos(h=8, w=w, x=x, y=y)
  + { id: id };

// ---- 面板组装 ----
// 耗时面板单位统一 ms:Prometheus 原始单位为秒,故 ×1000(原 L1/L2 的 ×1000000 µs 已修正)
local panels = [
  p.row('缓存命中率 (hit_rate)', 1001, hitRowY),
  seriesPanel(
    'L1 命中率', 1002, 0, 1, 12, 'percent',
    'rate(cache:$cache:l1:hits[5m]) / (rate(cache:$cache:l1:hits[5m]) + rate(cache:$cache:l1:misses[5m])) * 100',
    'L1 命中率', hitRateThresholds
  ),
  seriesPanel(
    'L2 命中率', 1003, 12, 1, 12, 'percent',
    'rate(cache:$cache:l2:hits[5m]) / (rate(cache:$cache:l2:hits[5m]) + rate(cache:$cache:l2:misses[5m])) * 100',
    'L2 命中率', hitRateThresholds
  ),

  p.row('缓存耗时 (duration)', 1010, durationRowY),
  seriesPanel(
    'L1 查询耗时', 1004, 0, 10, 8, 'ms',
    'sum(rate(cache:$cache:l1:get:duration_seconds_sum[5m])) / sum(rate(cache:$cache:l1:get:duration_seconds_count[5m])) * 1000',
    'L1 平均耗时', p.greenThreshold()
  ),
  seriesPanel(
    'L2 查询耗时', 1005, 8, 10, 8, 'ms',
    'sum(rate(cache:$cache:l2:get:duration_seconds_sum[5m])) / sum(rate(cache:$cache:l2:get:duration_seconds_count[5m])) * 1000',
    'L2 平均耗时', p.greenThreshold()
  ),
  seriesPanel(
    '数据库加载耗时', 1006, 16, 10, 8, 'ms',
    'sum(rate(cache:$cache:db:load:duration_seconds_sum[5m])) / sum(rate(cache:$cache:db:load:duration_seconds_count[5m])) * 1000',
    'DB 平均耗时', p.greenThreshold()
  ),

  p.row('缓存容量与穿透 (capacity)', 1020, capacityRowY),
  seriesPanel(
    'L1 条目数', 1007, 0, 19, 12, 'short',
    'cache:$cache:l1:entries',
    'L1 条目数', p.greenThreshold()
  ),
  seriesPanel(
    '数据库穿透加载', 1008, 12, 19, 12, 'reqps',
    'rate(cache:$cache:db:loads[5m])',
    '穿透加载', p.greenThreshold()
  ),
];

// ---- 模板变量:缓存实例 ----
local cacheVariable = {
  current: { selected: true, text: 'media_info', value: 'media_info' },
  hide: 0,
  label: '缓存实例',
  name: 'cache',
  options: [
    { selected: false, text: 'user_info', value: 'user_info' },
    { selected: false, text: 'user_info_single', value: 'user_info_single' },
    { selected: false, text: 'media_info', value: 'media_info' },
    { selected: false, text: 'media_dimensions', value: 'media_dimensions' },
    { selected: false, text: 'timeline_stat', value: 'timeline_stat' },
    { selected: false, text: 'person', value: 'person' },
  ],
  skipUrlSync: false,
  type: 'custom',
};

// ---- dashboard 装配 ----

local dashboard =
  g.dashboard.new('多级缓存监控')
  + g.dashboard.withUid('cache-monitoring')
  + g.dashboard.withTags(['cache', 'monitoring'])
  + g.dashboard.withEditable(true)
  + g.dashboard.withFiscalYearStartMonth(0)
  + g.dashboard.withLinks([])
  + g.dashboard.withRefresh('5s')
  + g.dashboard.withSchemaVersion(39)
  + g.dashboard.withTemplating({ list: [cacheVariable] })
  + g.dashboard.withTimezone('')
  + g.dashboard.time.withFrom('now-5m')
  + g.dashboard.time.withTo('now')
  + g.dashboard.withPanels(panels, setPanelIDs=false)
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
