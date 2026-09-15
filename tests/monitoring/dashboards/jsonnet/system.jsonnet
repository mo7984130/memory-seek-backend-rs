// 系统监控 dashboard
// 渲染:jsonnet -J vendor system.jsonnet | jq -S 'del(.panels[].pluginVersion)' > ../system.json
local g = import 'g.libsonnet';
local p = import 'lib/panels.libsonnet';

local ts = g.panel.timeSeries;
local stat = g.panel.stat;

local DS_UID = 'Prometheus';

// 绝对阈值(与面板类型无关的片段)
local thresholds(steps) =
  ts.standardOptions.thresholds.withMode('absolute')
  + ts.standardOptions.thresholds.withSteps(steps);

local green = thresholds([{ color: 'green', value: null }]);

// 图例不可见(showLegend: false),与设计规范及模块 dashboard 一致;
// 不用 displayMode: hidden(grafana 13.2.1 NoData 渲染 bug)
local legend = p.withLegendTooltip();

// 多查询 timeseries 面板(targets 为 [{expr, legend}] 列表)
local seriesPanel(title, id, x, y, w, unit, targets, targetThresholds=green) =
  ts.new(title)
  + ts.queryOptions.withDatasource('prometheus', DS_UID)
  + ts.queryOptions.withTargets(
    std.mapWithIndex(
      function(i, t) p.bareTarget(t.expr, t.legend, std.char(65 + i)),
      targets
    )
  )
  + p.baseCustom()
  + legend
  + ts.standardOptions.withUnit(unit)
  + targetThresholds
  + ts.panelOptions.withGridPos(h=8, w=w, x=x, y=y)
  + { id: id };

// 单值 stat 面板:阈值着色,取 lastNotNull
local statPanel(title, id, x, y, w, expr, legendFormat) =
  stat.new(title)
  + stat.queryOptions.withDatasource('prometheus', DS_UID)
  + stat.queryOptions.withTargets([p.bareTarget(expr, legendFormat, 'A')])
  + stat.options.withColorMode('value')
  + stat.options.withGraphMode('none')
  + stat.options.withJustifyMode('auto')
  + stat.options.withOrientation('auto')
  + stat.options.reduceOptions.withCalcs(['lastNotNull'])
  + stat.options.reduceOptions.withFields('')
  + stat.options.reduceOptions.withValues(false)
  + stat.options.withTextMode('auto')
  + stat.standardOptions.color.withMode('thresholds')
  + thresholds([
      { color: 'green', value: null },
      { color: 'yellow', value: 80 },
      { color: 'red', value: 90 },
    ])
  + stat.standardOptions.withUnit('short')
  + stat.standardOptions.withDecimals(0)
  + stat.panelOptions.withGridPos(h=8, w=w, x=x, y=y)
  + { id: id };

// ---- 面板组装 ----
local panels = [
  p.row('HTTP 请求 (http)', 1004, 0),
  seriesPanel(
    'HTTP QPS', 1001, 0, 1, 8, 'reqps',
    [{ expr: 'sum(rate(server_http_requests_total[5m])) by (route, method) > 0', legend: '{{method}} {{route}}' }]
  ),
  seriesPanel(
    'HTTP 错误率', 1002, 8, 1, 8, 'percent',
    [{ expr: 'sum(rate(server_http_requests_total{status_class="5xx"}[5m])) / sum(rate(server_http_requests_total[5m])) * 100', legend: '错误率' }],
    thresholds([
      { color: 'green', value: null },
      { color: 'yellow', value: 1 },
      { color: 'red', value: 5 },
    ])
  ),
  // 延迟只看 P99 汇总(详细分位数见各业务操作的耗时面板)
  seriesPanel(
    'HTTP 延迟 (P99)', 1003, 16, 1, 8, 'ms',
    [
      { expr: 'histogram_quantile(0.99, sum(rate(server_http_duration_seconds_bucket[5m])) by (le, route, method)) * 1000', legend: '{{method}} {{route}}' },
    ]
  ),

  p.row('CPU 监控', 1005, 2),
  seriesPanel(
    '系统 CPU 使用率', 1006, 0, 3, 8, 'percent',
    [{ expr: 'system_cpu_usage', legend: 'CPU' }],
    thresholds([
      { color: 'green', value: null },
      { color: 'yellow', value: 70 },
      { color: 'red', value: 90 },
    ])
  ),
  seriesPanel(
    '进程 CPU 使用率', 1007, 8, 3, 8, 'percent',
    [{ expr: 'system_cpu_process_usage', legend: '进程 CPU' }]
  ),
  statPanel('CPU 核心数', 1008, 16, 3, 6, 'system_cpu_cores', 'cores'),

  p.row('内存监控', 1009, 11),
  seriesPanel(
    '系统内存使用', 1010, 0, 12, 8, 'percent',
    [{ expr: 'system_memory_used / system_memory_total * 100', legend: '内存使用率' }]
  ),
  seriesPanel(
    '系统总内存', 1011, 8, 12, 8, 'decbytes',
    [{ expr: 'system_memory_total', legend: '总内存' }]
  ),
  seriesPanel(
    '进程内存使用', 1012, 16, 12, 8, 'decbytes',
    [{ expr: 'system_memory_process_usage', legend: '进程内存' }]
  ),

  p.row('磁盘监控', 1013, 20),
  seriesPanel(
    '磁盘总容量', 1014, 0, 21, 12, 'decbytes',
    [{ expr: 'system_disk_total', legend: '总容量' }]
  ),
  seriesPanel(
    '磁盘已用', 1015, 12, 21, 12, 'decbytes',
    [{ expr: 'system_disk_used', legend: '已用' }]
  ),

  p.row('数据库连接池', 1016, 29),
  seriesPanel(
    '活跃连接数', 1017, 0, 30, 8, 'short',
    [{ expr: 'database_connections_active', legend: '活跃' }]
  ),
  seriesPanel(
    '空闲连接数', 1018, 8, 30, 8, 'short',
    [{ expr: 'database_connections_idle', legend: '空闲' }]
  ),
  statPanel('最大连接数', 1019, 16, 30, 6, 'database_connections_max', 'max'),

  p.row('Redis 连接池', 1020, 38),
  seriesPanel(
    '活跃连接数', 1021, 0, 39, 8, 'short',
    [{ expr: 'redis_connections_active', legend: '活跃' }]
  ),
  seriesPanel(
    '空闲连接数', 1022, 8, 39, 8, 'short',
    [{ expr: 'redis_connections_idle', legend: '空闲' }]
  ),
  seriesPanel(
    '等待连接数', 1023, 16, 39, 8, 'short',
    [{ expr: 'redis_connections_waiting', legend: '等待' }]
  ),

  p.row('版本信息', 1024, 47),
  statPanel('构建版本', 1025, 0, 48, 6, 'server_build_info', '{{version}} {{commit}}'),
];

// ---- dashboard 装配 ----

local dashboard =
  g.dashboard.new('系统监控')
  + g.dashboard.withUid('system-monitoring')
  + g.dashboard.withTags(['system', 'monitoring'])
  + g.dashboard.withEditable(true)
  + g.dashboard.withFiscalYearStartMonth(0)
  + g.dashboard.withLinks([])
  + g.dashboard.withRefresh('5s')
  + g.dashboard.withSchemaVersion(39)
  + g.dashboard.withTemplating({ list: [] })
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
