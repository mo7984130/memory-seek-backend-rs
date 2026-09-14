// Photo 模块监控 dashboard
// 渲染:sh generate.sh(输出到 ../photo.json)
local g = import 'g.libsonnet';
local p = import 'lib/panels.libsonnet';
local ops = import 'lib/ops.libsonnet';

// ---- 布局参数(与 dashboard-design-spec.md 一致)----
local httpRowY = 0;      // HTTP 汇总行:row y=0,面板 y=1
local httpPanelY = 1;
local opStartY = 2;      // 第一个操作 row 的 y(行间隔 9 = 面板高 8 + row 高 1)
local opStepY = 9;
local extraStepY = 12;   // face_compute 附加面板占用高度(stat 行 4 + 趋势行 8)

// ---- 分组标题行(保留现状:收藏夹 / 评论 两个分节)----
local sectionTitles = {
  get_collection_list: '收藏夹操作 (collection)',
  publish_comment: '评论操作 (comment)',
};

// ---- face_compute 附加面板(gauge / counter / 派生速率,保留原手写 JSON 的等价实现)----

local baseStatOptions = {
  colorMode: 'value',
  graphMode: 'none',
  justifyMode: 'auto',
  orientation: 'auto',
  reduceOptions: { calcs: ['lastNotNull'], fields: '', values: false },
  textMode: 'auto',
};

// stat 面板公共字段
local faceStat(id, title, expr, x, w, y, defaults, options) = {
  datasource: { type: 'prometheus', uid: 'Prometheus' },
  fieldConfig: { defaults: defaults, overrides: [] },
  gridPos: { h: 4, w: w, x: x, y: y },
  id: id,
  options: options,
  targets: [{ expr: expr, refId: 'A' }],
  title: title,
  type: 'stat',
};

// 趋势面板公共字段
local faceTrend(id, title, x, y, lineInterpolation, targets) = {
  datasource: { type: 'prometheus', uid: 'Prometheus' },
  fieldConfig: {
    defaults: {
      color: { mode: 'palette-classic' },
      custom: {
        axisBorderShow: false,
        axisCenteredZero: false,
        axisColorMode: 'text',
        axisLabel: '',
        axisPlacement: 'auto',
        barAlignment: 0,
        drawStyle: 'line',
        fillOpacity: 10,
        gradientMode: 'none',
        lineInterpolation: lineInterpolation,
        lineWidth: 2,
        pointSize: 5,
        showPoints: 'auto',
        spanNulls: false,
        stacking: { group: 'A', mode: 'none' },
      },
      mappings: [],
      thresholds: { mode: 'absolute', steps: [{ color: 'green', value: null }] },
      unit: 'short',
    },
    overrides: [],
  },
  gridPos: { h: 8, w: 12, x: x, y: y },
  id: id,
  options: {
    legend: { calcs: ['lastNotNull'], displayMode: 'list', placement: 'bottom' },
    tooltip: { mode: 'single', sort: 'none' },
  },
  targets: targets,
  title: title,
  type: 'timeseries',
};

local faceExtras(y) = [
  // 运行状态 / 计算模式:gauge 映射为中文状态
  faceStat(5101, '运行状态', 'photo:face_compute:running', 0, 4, y, {
    color: { mode: 'thresholds' },
    mappings: [
      {
        options: {
          '0': { color: 'green', text: '空闲' },
          '1': { color: 'orange', text: '计算中' },
        },
        type: 'value',
      },
    ],
    thresholds: {
      mode: 'absolute',
      steps: [{ color: 'green', value: null }, { color: 'orange', value: 1 }],
    },
  }, baseStatOptions { colorMode: 'background' }),
  faceStat(5102, '计算模式', 'photo:face_compute:mode', 4, 4, y, {
    color: { mode: 'thresholds' },
    mappings: [
      {
        options: {
          '0': { color: 'blue', text: '增量' },
          '1': { color: 'purple', text: '全量' },
        },
        type: 'value',
      },
    ],
    thresholds: {
      mode: 'absolute',
      steps: [{ color: 'blue', value: null }, { color: 'purple', value: 1 }],
    },
  }, baseStatOptions { colorMode: 'background' }),
  faceStat(5103, '当前批次', 'photo:face_compute:batch', 8, 4, y, {
    color: { mode: 'thresholds' },
    mappings: [],
    thresholds: { mode: 'absolute', steps: [{ color: 'green', value: null }] },
  }, baseStatOptions),
  // 累计 counter
  faceStat(5104, '累计照片', 'photo:face_compute:total_photos', 12, 3, y, {
    color: { mode: 'thresholds' },
    mappings: [],
    thresholds: { mode: 'absolute', steps: [{ color: 'green', value: null }] },
    unit: 'short',
  }, baseStatOptions { graphMode: 'area' }),
  faceStat(5105, '累计人脸', 'photo:face_compute:total_faces', 15, 3, y, {
    color: { mode: 'thresholds' },
    mappings: [],
    thresholds: { mode: 'absolute', steps: [{ color: 'green', value: null }] },
    unit: 'short',
  }, baseStatOptions { graphMode: 'area' }),
  faceStat(5107, '无人脸数', 'photo:face_compute:total_no_face', 18, 3, y, {
    color: { mode: 'thresholds' },
    mappings: [],
    thresholds: { mode: 'absolute', steps: [{ color: 'green', value: null }] },
    unit: 'short',
  }, baseStatOptions { graphMode: 'area' }),
  // 派生速率 / 累计趋势
  faceTrend(5108, '处理速率', 0, y + 4, 'smooth', [
    { expr: 'rate(photo:face_compute:photos_processed[1m]) * 60', legendFormat: '处理照片/分钟', refId: 'A' },
    { expr: 'rate(photo:face_compute:faces_detected[1m]) * 60', legendFormat: '检出人脸/分钟', refId: 'B' },
    { expr: 'rate(photo:face_compute:no_face_photos[1m]) * 60', legendFormat: '无人脸/分钟', refId: 'C' },
  ]),
  faceTrend(5109, '累计趋势', 12, y + 4, 'stepAfter', [
    { expr: 'photo:face_compute:total_photos', legendFormat: '累计照片', refId: 'A' },
    { expr: 'photo:face_compute:total_faces', legendFormat: '累计人脸', refId: 'B' },
    { expr: 'photo:face_compute:total_no_face', legendFormat: '累计无人脸', refId: 'C' },
  ]),
];

// ---- 面板组装 ----

local httpPanels = [
  p.row('HTTP 请求 (http)', 2001, httpRowY),
  p.httpQpsPanel('photo', 2002, 0, httpPanelY),
  p.httpErrorRatePanel('photo', 2003, 8, httpPanelY),
  p.httpLatencyPanel('photo', 2004, 16, httpPanelY),
];

// 逐操作铺排:每操作 = row + 三件套;无子步骤的操作省略子步骤面板并把耗时面板加宽为 12,
// face_compute 额外追加 gauge/counter/趋势面板。
local opBlocks = std.foldl(
  function(acc, op)
    local idx = acc.idx;
    local hasSection = std.objectHas(sectionTitles, op.func);
    local y0 = acc.y + (if hasSection then 1 else 0);
    local idBase = (idx + 1) * 10;
    local hasSteps = std.length(op.steps) > 0;
    local isFaceCompute = op.func == 'face_compute';

    local sectionRow =
      if hasSection then [p.row(sectionTitles[op.func], 5000 + idx, acc.y)] else [];

    local durationPanel =
      p.durationPanel(ops.photo.crate, op.name, op.func, idBase + 2, 0, y0 + 1)
      + (if hasSteps then {} else { gridPos: { h: 8, w: 12, x: 0, y: y0 + 1 } });

    local substepsPanels =
      if hasSteps then [p.substepsPanel(ops.photo.crate, op.name, op.func, op.steps, idBase + 3, 6, y0 + 1)]
      else [];

    local extraPanels = if isFaceCompute then faceExtras(y0 + opStepY) else [];

    {
      idx: idx + 1,
      y: y0 + opStepY + (if isFaceCompute then extraStepY else 0),
      panels:
        acc.panels
        + sectionRow
        + [
          p.row(op.rowTitle, idBase + 1, y0),
          durationPanel,
        ]
        + substepsPanels
        + [p.qpsSuccessPanel(ops.photo.crate, op.name, op.func, idBase + 4, 12, y0 + 1)]
        + extraPanels,
    },
  ops.photo.ops,
  { idx: 0, y: opStartY, panels: [] }
).panels;

// ---- dashboard 装配 ----

local dashboard =
  g.dashboard.new('Photo 模块监控')
  + g.dashboard.withUid('photo-module-monitoring')
  + g.dashboard.withTags(['photo', 'api', 'monitoring'])
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
