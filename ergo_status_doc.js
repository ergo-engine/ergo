const fs = require("fs");
const {
  Document, Packer, Paragraph, TextRun, Table, TableRow, TableCell,
  Header, Footer, AlignmentType, HeadingLevel, BorderStyle, WidthType,
  ShadingType, PageNumber, PageBreak, LevelFormat
} = require("docx");

const border = { style: BorderStyle.SINGLE, size: 1, color: "CCCCCC" };
const borders = { top: border, bottom: border, left: border, right: border };
const cellMargins = { top: 80, bottom: 80, left: 120, right: 120 };

function headerCell(text, width) {
  return new TableCell({
    borders,
    width: { size: width, type: WidthType.DXA },
    shading: { fill: "1B2A3D", type: ShadingType.CLEAR },
    margins: cellMargins,
    children: [new Paragraph({ children: [new TextRun({ text, bold: true, font: "Arial", size: 20, color: "FFFFFF" })] })]
  });
}

function cell(text, width) {
  return new TableCell({
    borders,
    width: { size: width, type: WidthType.DXA },
    margins: cellMargins,
    children: [new Paragraph({ children: [new TextRun({ text, font: "Arial", size: 20 })] })]
  });
}

function cellBold(text, width) {
  return new TableCell({
    borders,
    width: { size: width, type: WidthType.DXA },
    margins: cellMargins,
    children: [new Paragraph({ children: [new TextRun({ text, font: "Arial", size: 20, bold: true })] })]
  });
}

function statusCell(text, width, color) {
  return new TableCell({
    borders,
    width: { size: width, type: WidthType.DXA },
    shading: { fill: color, type: ShadingType.CLEAR },
    margins: cellMargins,
    children: [new Paragraph({
      alignment: AlignmentType.CENTER,
      children: [new TextRun({ text, bold: true, font: "Arial", size: 20, color: "FFFFFF" })]
    })]
  });
}

function p(text, opts = {}) {
  return new Paragraph({
    spacing: { after: 120 },
    children: [new TextRun({ text, font: "Arial", size: 22, ...opts })]
  });
}

function heading(text, level) {
  return new Paragraph({
    heading: level,
    spacing: { before: 300, after: 160 },
    children: [new TextRun({ text, font: "Arial" })]
  });
}

function multiRunParagraph(runs) {
  return new Paragraph({
    spacing: { after: 120 },
    children: runs.map(r => new TextRun({ font: "Arial", size: 22, ...r }))
  });
}

const doc = new Document({
  styles: {
    default: { document: { run: { font: "Arial", size: 22 } } },
    paragraphStyles: [
      {
        id: "Heading1", name: "Heading 1", basedOn: "Normal", next: "Normal", quickFormat: true,
        run: { size: 36, bold: true, font: "Arial", color: "1B2A3D" },
        paragraph: { spacing: { before: 360, after: 240 }, outlineLevel: 0 }
      },
      {
        id: "Heading2", name: "Heading 2", basedOn: "Normal", next: "Normal", quickFormat: true,
        run: { size: 28, bold: true, font: "Arial", color: "2E5090" },
        paragraph: { spacing: { before: 240, after: 160 }, outlineLevel: 1 }
      },
      {
        id: "Heading3", name: "Heading 3", basedOn: "Normal", next: "Normal", quickFormat: true,
        run: { size: 24, bold: true, font: "Arial", color: "444444" },
        paragraph: { spacing: { before: 200, after: 120 }, outlineLevel: 2 }
      },
    ]
  },
  numbering: {
    config: [
      {
        reference: "bullets",
        levels: [{
          level: 0, format: LevelFormat.BULLET, text: "\u2022", alignment: AlignmentType.LEFT,
          style: { paragraph: { indent: { left: 720, hanging: 360 } } }
        }]
      },
      {
        reference: "numbers",
        levels: [{
          level: 0, format: LevelFormat.DECIMAL, text: "%1.", alignment: AlignmentType.LEFT,
          style: { paragraph: { indent: { left: 720, hanging: 360 } } }
        }]
      },
    ]
  },
  sections: [
    // ---- TITLE PAGE ----
    {
      properties: {
        page: {
          size: { width: 12240, height: 15840 },
          margin: { top: 1440, right: 1440, bottom: 1440, left: 1440 }
        }
      },
      children: [
        new Paragraph({ spacing: { before: 3600 } }),
        new Paragraph({
          alignment: AlignmentType.CENTER,
          spacing: { after: 200 },
          children: [new TextRun({ text: "Ergo", font: "Arial", size: 56, bold: true, color: "1B2A3D" })]
        }),
        new Paragraph({
          alignment: AlignmentType.CENTER,
          spacing: { after: 600 },
          children: [new TextRun({ text: "Project Status and Next Steps", font: "Arial", size: 32, color: "666666" })]
        }),
        new Paragraph({
          alignment: AlignmentType.CENTER,
          spacing: { after: 100 },
          children: [new TextRun({ text: "February 2026", font: "Arial", size: 22, color: "888888" })]
        }),
        new Paragraph({
          alignment: AlignmentType.CENTER,
          children: [new TextRun({ text: "Prepared by Claude (Structural Auditor)", font: "Arial", size: 20, color: "888888", italics: true })]
        }),
        new Paragraph({ children: [new PageBreak()] }),
      ]
    },
    // ---- MAIN CONTENT ----
    {
      properties: {
        page: {
          size: { width: 12240, height: 15840 },
          margin: { top: 1440, right: 1440, bottom: 1440, left: 1440 }
        }
      },
      headers: {
        default: new Header({
          children: [new Paragraph({
            alignment: AlignmentType.RIGHT,
            children: [new TextRun({ text: "Ergo \u2014 Status Report", font: "Arial", size: 16, color: "999999", italics: true })]
          })]
        })
      },
      footers: {
        default: new Footer({
          children: [new Paragraph({
            alignment: AlignmentType.CENTER,
            children: [
              new TextRun({ text: "Page ", font: "Arial", size: 16, color: "999999" }),
              new TextRun({ children: [PageNumber.CURRENT], font: "Arial", size: 16, color: "999999" }),
            ]
          })]
        })
      },
      children: [
        // ==== 1. THE THREE LAYERS ====
        heading("1. Architecture: Three Layers", HeadingLevel.HEADING_1),

        p("Ergo is built as three distinct layers, each with a clearly defined responsibility. This separation is load-bearing and is enforced by the frozen specifications."),

        new Table({
          width: { size: 9360, type: WidthType.DXA },
          columnWidths: [2000, 3680, 3680],
          rows: [
            new TableRow({ children: [
              headerCell("Layer", 2000),
              headerCell("Responsibility", 3680),
              headerCell("Key Principle", 3680),
            ]}),
            new TableRow({ children: [
              cellBold("Runtime", 2000),
              cell("Deterministic DAG execution. Takes an expanded graph and runs it once. Numbers flow through nodes, actions fire, outputs produced.", 3680),
              cell("Pure computation. No scheduling, no state across runs, no awareness of time or events.", 3680),
            ]}),
            new TableRow({ children: [
              cellBold("Supervisor", 2000),
              cell("Mechanical scheduler. Receives external events, applies constraints (rate limits, concurrency, deadlines), decides when to invoke runtime.run().", 3680),
              cell("Strategy-neutral (SUP-2). Never inspects graph outputs. Only sees RunTermination. Decisions are replayable (SUP-3).", 3680),
            ]}),
            new TableRow({ children: [
              cellBold("Scenario Planner", 2000),
              cell("Multi-graph selection, parameter sweeps, A/B scenarios. Would sit above the supervisor.", 3680),
              cell("Out of scope for v0. Does not exist yet.", 3680),
            ]}),
          ]
        }),

        new Paragraph({ spacing: { after: 80 } }),

        multiRunParagraph([
          { text: "The boundary between supervisor and runtime is structural, not advisory. The supervisor calls " },
          { text: "runtime.run()", italics: true },
          { text: " through the " },
          { text: "RuntimeInvoker", italics: true },
          { text: " trait and only receives a " },
          { text: "RunTermination", italics: true },
          { text: " back (Completed, TimedOut, Aborted, or Failed). It never sees what the graph computed. This prevents the supervisor from becoming a policy engine." },
        ]),

        p("Causality between episodes flows through the environment: Episode N writes to an external store via Actions, Episode N+1 reads from that store via Sources. The supervisor cannot inject state from one episode into the next (CXT-1)."),

        // ==== 2. WHAT WE BUILT ====
        heading("2. What We Built: The YAML Graph Interface", HeadingLevel.HEADING_1),

        p("The YAML graph interface is the first-class authoring format for ergo graphs. It replaces the need to construct ClusterDefinition structs in Rust code by hand. A user writes a .yaml file describing their graph, and the system parses, validates, expands, and executes it."),

        heading("What the parser handles", HeadingLevel.HEADING_2),

        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "Packed node references: impl: add@0.1.0 or cluster: my_cluster@1.0", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "Shorthand edge syntax: src.value -> add.a (also supports structured object form)", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "External input syntax: $threshold -> gt.b with validation against declared inputs", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "Inferred parameter bindings: scalar values become literals, { exposed: name } becomes exposed", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "Convention-based cluster resolution: same directory, ./clusters/ subdirectory, or --cluster-path search paths", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "Recursive cluster tree loading with circular import detection", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "Version coercion (YAML 1.0 as float survives), identifier validation (no dots, no @, no $, no spaces)", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "Optional adapter validation via --adapter flag", font: "Arial", size: 22 })]
        }),

        heading("Implementation", HeadingLevel.HEADING_2),

        new Table({
          width: { size: 9360, type: WidthType.DXA },
          columnWidths: [3680, 1800, 3880],
          rows: [
            new TableRow({ children: [
              headerCell("File", 3680),
              headerCell("Lines", 1800),
              headerCell("Role", 3880),
            ]}),
            new TableRow({ children: [
              cell("crates/ergo-cli/src/graph_yaml.rs", 3680),
              cell("1,511", 1800),
              cell("YAML parser, cluster loader, adapter validation, execution, all tests", 3880),
            ]}),
            new TableRow({ children: [
              cell("crates/ergo-cli/src/main.rs", 3680),
              cell("Modified", 1800),
              cell("CLI wiring: ergo run <graph.yaml> falls through to graph_yaml module", 3880),
            ]}),
          ]
        }),

        new Paragraph({ spacing: { after: 80 } }),

        heading("Test coverage", HeadingLevel.HEADING_2),

        p("28 tests pass for ergo-cli (12 unit tests in src/main.rs, including 11 graph_yaml scenarios, plus 16 existing phase7_cli tests). The critical test is demo_1_yaml_executes_end_to_end: a full 15-node graph parsed from a YAML string literal, expanded, run through the runtime, with 4 output values asserted (sum_left=6.0, sum_total=8.0, action_a=Completed, action_b=Skipped)."),

        heading("CLI interface", HeadingLevel.HEADING_2),

        new Paragraph({
          spacing: { after: 120 },
          shading: { fill: "F0F0F0", type: ShadingType.CLEAR },
          children: [new TextRun({ text: "  ergo run <graph.yaml> [--adapter <adapter.yaml>] [--cluster-path <path> ...]", font: "Courier New", size: 20 })]
        }),

        // ==== 3. THE GAP ====
        heading("3. The Gap: No Supervisor Path for YAML Graphs", HeadingLevel.HEADING_1),

        p("This is the central issue. The YAML graph interface currently bypasses the supervisor entirely."),

        heading("Current execution paths", HeadingLevel.HEADING_2),

        new Table({
          width: { size: 9360, type: WidthType.DXA },
          columnWidths: [1800, 3780, 3780],
          rows: [
            new TableRow({ children: [
              headerCell("Path", 1800),
              headerCell("What Happens", 3780),
              headerCell("Supervisor?", 3780),
            ]}),
            new TableRow({ children: [
              cellBold("ergo run demo-1", 1800),
              cell("Hardcoded graph built in Rust. Fed through Supervisor with CapturingSession. Events are hardcoded ExternalEvent instances. Decision log captured. Replay verification works.", 3780),
              statusCell("YES", 3780, "238636"),
            ]}),
            new TableRow({ children: [
              cellBold("ergo run fixture <path>", 1800),
              cell("Same hardcoded graph. Events come from a .jsonl fixture file. Goes through Supervisor. Decision log captured.", 3780),
              statusCell("YES", 3780, "238636"),
            ]}),
            new TableRow({ children: [
              cellBold("ergo run graph.yaml", 1800),
              cell("YAML parsed and expanded. Calls runtime::run() directly. Single execution. No events, no episodes, no decision log, no replay.", 3780),
              statusCell("NO", 3780, "DA3633"),
            ]}),
          ]
        }),

        new Paragraph({ spacing: { after: 80 } }),

        multiRunParagraph([
          { text: "The YAML path produces an " },
          { text: "ExpandedGraph", italics: true },
          { text: " \u2014 which is exactly what " },
          { text: "Supervisor::new()", italics: true },
          { text: " accepts. The data is ready. The plumbing to connect them does not exist yet." },
        ]),

        heading("What the YAML path is missing", HeadingLevel.HEADING_2),

        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "Episode multiplicity: same graph, repeated invocations with fresh context each time", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "Mechanical scheduling: defer/invoke decisions based on constraints", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "Decision logging: append-only record of every scheduling decision (SUP-3, SUP-7)", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "Capture and replay: deterministic replay of scheduling decisions from captured bundles", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "Constraint enforcement: rate limits, concurrency caps, deadlines, mechanical retries", font: "Arial", size: 22 })]
        }),

        // ==== 4. WHAT NEEDS TO HAPPEN ====
        heading("4. What Needs to Happen", HeadingLevel.HEADING_1),

        heading("The canonical execution path", HeadingLevel.HEADING_2),

        p("The target architecture is one execution path. Live and replay are the same codepath. The only variable is where events originate."),

        new Paragraph({
          numbering: { reference: "numbers", level: 0 },
          spacing: { after: 80 },
          children: [new TextRun({ text: "Parse YAML into ClusterDefinition (graph_yaml.rs \u2014 exists)", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "numbers", level: 0 },
          spacing: { after: 80 },
          children: [new TextRun({ text: "Load cluster tree and expand into ExpandedGraph (graph_yaml.rs \u2014 exists)", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "numbers", level: 0 },
          spacing: { after: 80 },
          children: [new TextRun({ text: "Construct RuntimeHandle from the expanded graph, catalog, and registries (supervisor lib.rs \u2014 exists)", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "numbers", level: 0 },
          spacing: { after: 80 },
          children: [new TextRun({ text: "Construct CapturingSession(Supervisor) with the RuntimeHandle and constraints (supervisor lib.rs \u2014 exists)", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "numbers", level: 0 },
          spacing: { after: 80 },
          children: [new TextRun({ text: "Feed ExternalEvent instances via on_event() \u2014 from live adapter or replayed bundle (needs wiring)", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "numbers", level: 0 },
          spacing: { after: 80 },
          children: [new TextRun({ text: "Capture decisions into CaptureBundle for replay verification (supervisor capture.rs \u2014 exists)", font: "Arial", size: 22 })]
        }),

        new Paragraph({ spacing: { after: 80 } }),

        multiRunParagraph([
          { text: "Steps 1\u20134 and 6 already exist as working code. Step 5 is the integration gap. ", bold: true },
          { text: "The event source is the open design question: for a YAML-defined graph, where do ExternalEvent instances come from?" },
        ]),

        heading("Event source: the design question", HeadingLevel.HEADING_2),

        p("The supervisor receives events; it does not generate them. Per the architecture, the adapter layer provides execution context and captures events for replay. For YAML graphs, the event source options are:"),

        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "Fixture files (.jsonl): Already exist as a format. Sequential episode_start + event entries. Deterministic by construction. Currently only used with the hardcoded demo_1 graph.", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "Live adapter: A real-world event source (webhook listener, file watcher, timer). Events get captured into a CaptureBundle. This is the production path.", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "Replayed bundle: Rehydrate ExternalEvent instances from a previously captured CaptureBundle. The replay path (replay.rs, replay_checked()) already does this.", font: "Arial", size: 22 })]
        }),

        new Paragraph({ spacing: { after: 80 } }),

        p("All three feed ExternalEvent instances into the same Supervisor::on_event() method. The supervisor does not know or care which source is active. This is the determinism guarantee: identical event streams produce identical scheduling decisions."),

        heading("What does not need to change", HeadingLevel.HEADING_2),

        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "The YAML parser (graph_yaml.rs) \u2014 it already produces ClusterDefinition and ExpandedGraph correctly", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "The supervisor (lib.rs) \u2014 it already accepts ExpandedGraph and processes events mechanically", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "The runtime \u2014 deterministic DAG execution is complete", font: "Arial", size: 22 })]
        }),
        new Paragraph({
          numbering: { reference: "bullets", level: 0 },
          spacing: { after: 60 },
          children: [new TextRun({ text: "The capture/replay system \u2014 CaptureBundle format and replay_checked() work", font: "Arial", size: 22 })]
        }),

        heading("Current direct-execution path", HeadingLevel.HEADING_2),

        multiRunParagraph([
          { text: "The current " },
          { text: "ergo run graph.yaml", italics: true },
          { text: " path that calls " },
          { text: "runtime::run()", italics: true },
          { text: " directly should be treated as a non-canonical debug mode, not the primary execution path. It is useful for quick validation during development but does not satisfy the execution model." },
        ]),

        // ==== 5. GOVERNING PRINCIPLES ====
        heading("5. Governing Principles", HeadingLevel.HEADING_1),

        p("These are not suggestions. They are frozen specifications that constrain the implementation."),

        new Table({
          width: { size: 9360, type: WidthType.DXA },
          columnWidths: [1400, 3280, 4680],
          rows: [
            new TableRow({ children: [
              headerCell("Invariant", 1400),
              headerCell("Rule", 3280),
              headerCell("Why It Matters Here", 4680),
            ]}),
            new TableRow({ children: [
              cellBold("CXT-1", 1400),
              cell("ExecutionContext is adapter-only. No supervisor-derived state.", 3280),
              cell("The supervisor cannot inject results from Episode N into Episode N+1. Cross-episode causality flows through the environment via Actions and Sources.", 4680),
            ]}),
            new TableRow({ children: [
              cellBold("SUP-1", 1400),
              cell("Supervisor is graph-identity fixed.", 3280),
              cell("One supervisor instance, one graph. To run a different YAML graph, construct a new supervisor.", 4680),
            ]}),
            new TableRow({ children: [
              cellBold("SUP-2", 1400),
              cell("Supervisor is strategy-neutral.", 3280),
              cell("The supervisor only sees RunTermination, never RunResult. It cannot make domain-aware decisions. Policy belongs in the graph.", 4680),
            ]}),
            new TableRow({ children: [
              cellBold("SUP-3", 1400),
              cell("Supervisor decisions are replayable.", 3280),
              cell("Replay scope is scheduling decisions, not full output equivalence. Same event stream must produce same invoke/defer/skip sequence.", 4680),
            ]}),
            new TableRow({ children: [
              cellBold("SUP-4", 1400),
              cell("Retries only on mechanical failure.", 3280),
              cell("NetworkTimeout and AdapterUnavailable are retryable. Domain outcomes (order rejected, insufficient funds) are not.", 4680),
            ]}),
            new TableRow({ children: [
              cellBold("SUP-7", 1400),
              cell("DecisionLog is write-only.", 3280),
              cell("The supervisor emits log entries but cannot read them. Logging is mandatory, not optional.", 4680),
            ]}),
          ]
        }),

        new Paragraph({ spacing: { after: 80 } }),

        p("Policy is authored in the graph via compute, trigger, and action primitives. The supervisor is mechanical orchestration. Multi-graph policy belongs to the Scenario Planner layer, which is out of scope for v0."),

        // ==== 6. KNOWN LIMITATIONS ====
        heading("6. Known Limitations (Not Blocking)", HeadingLevel.HEADING_1),

        new Table({
          width: { size: 9360, type: WidthType.DXA },
          columnWidths: [3200, 4360, 1800],
          rows: [
            new TableRow({ children: [
              headerCell("Limitation", 3200),
              headerCell("Detail", 4360),
              headerCell("Status", 1800),
            ]}),
            new TableRow({ children: [
              cellBold("validate_declared_signature is wireability-only", 3200),
              cell("Does not check kind, has_side_effects, is_origin, or port subset compatibility. Only checks that declared wireability does not exceed inferred wireability.", 4360),
              cell("Acknowledged", 1800),
            ]}),
            new TableRow({ children: [
              cellBold("Version constraints are exact-match only", 3200),
              cell("TODO(I.6) in cluster.rs. No semver parsing, no range resolution. Catalog and cluster loader do literal string match on version.", 4360),
              cell("Acknowledged", 1800),
            ]}),
            new TableRow({ children: [
              cellBold("Decision::Skip is unused", 3200),
              cell("The enum variant exists but is never emitted by the current supervisor implementation. Only Invoke and Defer are produced.", 4360),
              cell("Acknowledged", 1800),
            ]}),
            new TableRow({ children: [
              cellBold("YAML CLI path bypasses supervisor", 3200),
              cell("ergo run graph.yaml currently executes runtime::run() directly. It does not yet run through Supervisor/CapturingSession for event-driven episodes, decision logging, and replay capture.", 4360),
              cell("Acknowledged", 1800),
            ]}),
          ]
        }),
      ]
    }
  ]
});

Packer.toBuffer(doc).then(buffer => {
  fs.writeFileSync("ergo_status_report.docx", buffer);
  console.log("Done: ergo_status_report.docx");
});
