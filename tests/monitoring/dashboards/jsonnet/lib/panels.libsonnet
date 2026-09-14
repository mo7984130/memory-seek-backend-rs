// 面板模板库:HTTP 汇总行 + 每操作标准三件套(耗时 / 子步骤耗时 / 调用量成功率)
// 样式与 tests/monitoring/dashboards/dashboard-design-spec.md 保持一致
local g = import '../g.libsonnet';

{
  local ts = g.panel.timeSeries,
  local rowPanel = g.panel.row,
  local prom = g.query.prometheus,

  local DS_UID = 'Prometheus',

  // ---- 查询构造 ----

  // 业务 target:与现状导出一致,target 上带 datasource
  promTarget(expr, legend, refId):
    prom.new(DS_UID, expr)
    + prom.withLegendFormat(legend)
    + prom.withRefId(refId),

  // 不带 target 级 datasource(HTTP 行现状如此,继承 panel 级)
  bareTarget(expr, legend, refId): {
    expr: expr,
    legendFormat: legend,
    refId: refId,
  },

  // ---- 公共 options ----

  // 公共 options:图例隐藏(showLegend: false)。
  // 注意:不要用 displayMode: "hidden" —— Grafana 13 对其有渲染 bug,面板会显示 NoData;
  // list + showLegend: false 既不可见图例也不会触发 bug。
  withLegendTooltip():
    ts.options.legend.withCalcs([])
    + ts.options.legend.withDisplayMode('list')
    + ts.options.legend.withShowLegend(false)
    + ts.options.legend.withPlacement('bottom')
    + ts.options.tooltip.withMode('single')
    + ts.options.tooltip.withSort('none'),

  // 基础折线样式 + palette-classic 配色
  baseCustom(lineWidth=2, fillOpacity=10, showPoints='auto'):
    ts.fieldConfig.defaults.custom.withDrawStyle('line')
    + ts.fieldConfig.defaults.custom.withLineInterpolation('smooth')
    + ts.fieldConfig.defaults.custom.withLineWidth(lineWidth)
    + ts.fieldConfig.defaults.custom.withFillOpacity(fillOpacity)
    + ts.fieldConfig.defaults.custom.withShowPoints(showPoints)
    + ts.fieldConfig.defaults.custom.stacking.withGroup('A')
    + ts.fieldConfig.defaults.custom.stacking.withMode('none')
    + ts.standardOptions.color.withMode('palette-classic'),

  greenThreshold():
    ts.standardOptions.thresholds.withMode('absolute')
    + ts.standardOptions.thresholds.withSteps([
      { color: 'green', value: null },
    ]),

  // 耗时阈值:<100 绿 / 100-500 黄 / >500 红
  latencyThresholds():
    ts.standardOptions.thresholds.withMode('absolute')
    + ts.standardOptions.thresholds.withSteps([
      { color: 'green', value: null },
      { color: 'yellow', value: 100 },
      { color: 'red', value: 500 },
    ]),

  // ---- row ----

  row(title, id, y):
    rowPanel.new(title)
    + rowPanel.withCollapsed(false)
    + rowPanel.withPanels([])
    + rowPanel.gridPos.withH(1)
    + rowPanel.gridPos.withW(24)
    + rowPanel.gridPos.withX(0)
    + rowPanel.gridPos.withY(y)
    + { id: id },

  // ---- HTTP 汇总行面板(module: 该 dashboard 所属模块,查询按模块过滤)----

  httpQpsPanel(module, id, x, y):
    ts.new('HTTP QPS')
    + ts.queryOptions.withDatasource('prometheus', DS_UID)
    + ts.queryOptions.withTargets([
      self.bareTarget(
        'sum(rate(server_http_requests_total{module="' + module + '"}[5m])) by (route)',
        '{{route}}',
        'A'
      ),
    ])
    + self.baseCustom()
    + self.greenThreshold()
    + ts.standardOptions.withUnit('reqps')
    + self.withLegendTooltip()
    + ts.panelOptions.withGridPos(h=8, w=8, x=x, y=y)
    + { id: id },

  httpErrorRatePanel(module, id, x, y):
    ts.new('HTTP 错误率')
    + ts.queryOptions.withDatasource('prometheus', DS_UID)
    + ts.queryOptions.withTargets([
      self.bareTarget(
        'sum(rate(server_http_requests_total{status_class="5xx",module="' + module + '"}[5m])) / sum(rate(server_http_requests_total{module="' + module + '"}[5m])) * 100',
        '错误率',
        'A'
      ),
    ])
    + self.baseCustom()
    + self.greenThreshold()
    + ts.standardOptions.withUnit('percent')
    + self.withLegendTooltip()
    + ts.panelOptions.withGridPos(h=8, w=8, x=x, y=y)
    + { id: id },

  // HTTP 延迟汇总:P99(详细分位数在各业务操作的耗时面板)单查询
  httpLatencyPanel(module, id, x, y):
    ts.new('HTTP 延迟 (P99)')
    + ts.queryOptions.withDatasource('prometheus', DS_UID)
    + ts.queryOptions.withTargets([
      self.bareTarget(
        'histogram_quantile(0.99, sum(rate(server_http_duration_seconds_bucket{module="' + module + '"}[5m])) by (le, route)) * 1000',
        '{{route}}',
        'A'
      ),
    ])
    + self.baseCustom()
    + self.greenThreshold()
    + ts.standardOptions.withUnit('ms')
    + self.withLegendTooltip()
    + ts.panelOptions.withGridPos(h=8, w=8, x=x, y=y)
    + { id: id },

  // ---- 标准三件套 ----

  // 耗时:P50 / P95 / P99 三分位,原始单位秒 ×1000 展示为 ms
  durationPanel(crate, name, metric, id, x, y):
    ts.new(name + '耗时')
    + ts.queryOptions.withDatasource('prometheus', DS_UID)
    + ts.queryOptions.withTargets([
      self.promTarget(
        'histogram_quantile(0.5, sum(rate(' + crate + ':' + metric + ':duration_seconds_bucket[5m])) by (le)) * 1000',
        'P50',
        'A'
      ),
      self.promTarget(
        'histogram_quantile(0.95, sum(rate(' + crate + ':' + metric + ':duration_seconds_bucket[5m])) by (le)) * 1000',
        'P95',
        'B'
      ),
      self.promTarget(
        'histogram_quantile(0.99, sum(rate(' + crate + ':' + metric + ':duration_seconds_bucket[5m])) by (le)) * 1000',
        'P99',
        'C'
      ),
    ])
    + self.baseCustom()
    + self.latencyThresholds()
    + ts.standardOptions.withUnit('ms')
    + self.withLegendTooltip()
    + ts.panelOptions.withGridPos(h=8, w=6, x=x, y=y)
    + { id: id },

  // 子步骤:平均耗时(_sum/_count ×1000),填充面积样式
  substepsPanel(crate, name, metric, steps, id, x, y):
    ts.new(name + '子步骤耗时')
    + ts.queryOptions.withDatasource('prometheus', DS_UID)
    + ts.queryOptions.withTargets(
      std.mapWithIndex(
        function(i, s)
          self.promTarget(
            'rate(' + crate + ':' + metric + ':' + s.metric + '_sum[5m]) / rate(' + crate + ':' + metric + ':' + s.metric + '_count[5m]) * 1000',
            s.label,
            std.char(65 + i)
          ),
        steps
      )
    )
    + self.baseCustom(lineWidth=1, fillOpacity=40, showPoints='never')
    + self.greenThreshold()
    + ts.standardOptions.withUnit('ms')
    + self.withLegendTooltip()
    + ts.panelOptions.withGridPos(h=8, w=6, x=x, y=y)
    + { id: id },

  // 调用量 + 成功率组合图:左轴 QPS,右轴成功率(override)
  qpsSuccessPanel(crate, name, metric, id, x, y):
    ts.new(name + '调用量成功率')
    + ts.queryOptions.withDatasource('prometheus', DS_UID)
    + ts.queryOptions.withTargets([
      self.promTarget(
        'rate(' + crate + ':' + metric + ':attempts[5m])',
        'QPS',
        'A'
      ),
      self.promTarget(
        'rate(' + crate + ':' + metric + ':success[5m]) / rate(' + crate + ':' + metric + ':attempts[5m]) * 100',
        '成功率',
        'B'
      ),
    ])
    + self.baseCustom()
    + self.greenThreshold()
    + ts.standardOptions.withUnit('reqps')
    + ts.standardOptions.withOverrides([
      ts.fieldOverride.byName.new('成功率')
      + ts.fieldOverride.byName.withProperty('custom.axisPlacement', 'right')
      + ts.fieldOverride.byName.withProperty('custom.axisLabel', '成功率')
      + ts.fieldOverride.byName.withProperty('unit', 'percent')
      + ts.fieldOverride.byName.withProperty('min', 0)
      + ts.fieldOverride.byName.withProperty('thresholds', {
        mode: 'absolute',
        steps: [
          { color: 'red', value: null },
          { color: 'yellow', value: 95 },
          { color: 'green', value: 99 },
        ],
      }),
    ])
    + self.withLegendTooltip()
    + ts.panelOptions.withGridPos(h=8, w=12, x=x, y=y)
    + { id: id },
}