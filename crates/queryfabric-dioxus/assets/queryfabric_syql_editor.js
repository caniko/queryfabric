/**
 * QueryFabric SyQL Interactive Editor.
 *
 * Loaded as an ES module. It looks for textareas annotated with
 * `data-queryfabric-syql-editor` and upgrades them into a CodeMirror editor.
 */

import { basicSetup } from "https://esm.sh/codemirror@6.0.1";
import { sql, SQLDialect } from "https://esm.sh/@codemirror/lang-sql";
import { autocompletion } from "https://esm.sh/@codemirror/autocomplete";
import { oneDark } from "https://esm.sh/@codemirror/theme-one-dark";
import { linter, lintGutter } from "https://esm.sh/@codemirror/lint";
import { EditorState, Compartment } from "https://esm.sh/@codemirror/state";
import { EditorView, keymap } from "https://esm.sh/@codemirror/view";

const DEFAULT_CATALOG_URL = "/static/queryfabric_catalog.json";
const DEFAULT_VALIDATE_URL = "/_ui/query/syql/validate";

function emptyCatalogData() {
  return {
    schema: {},
    tableNames: [],
    relationAliases: {},
    metadataFields: [],
  };
}

function normaliseCatalogArtifact(artifact) {
  const relations = artifact?.catalog?.relations || [];
  const schema = {};
  const relationAliases = {};

  for (const relation of relations) {
    schema[relation.name] = (relation.columns || []).map((column) => column.name);
    for (const alias of relation.aliases || []) {
      relationAliases[alias] = relation.name;
    }
  }

  return {
    schema,
    tableNames: Object.keys(schema),
    relationAliases,
    metadataFields: artifact?.metadata_fields || [],
  };
}

const catalogDataCache = new Map();

function loadCatalogData(catalogUrl) {
  const url = catalogUrl || DEFAULT_CATALOG_URL;
  if (!catalogDataCache.has(url)) {
    const promise = fetch(url)
      .then(async (response) => {
        if (!response.ok) {
          throw new Error(`catalog fetch failed with ${response.status}`);
        }
        return normaliseCatalogArtifact(await response.json());
      })
      .catch((error) => {
        console.error("Failed to load QueryFabric catalog artifact", error);
        return emptyCatalogData();
      });
    catalogDataCache.set(url, promise);
  }
  return catalogDataCache.get(url);
}

const KEYWORDS = [
  "FROM", "SELECT", "WHERE", "LIMIT", "AND", "OR", "NOT",
  "IN", "LIKE", "BETWEEN", "IS", "NULL", "TRUE", "FALSE",
  "ASC", "DESC", "ORDER", "BY",
];
const SCOPE_VALUES = ["local", "remote", "federation"];
const DOWNLOAD_VALUES = ["arrow", "parquet", "csv"];
const CLAUSES = ["SCOPE", "DOWNLOAD"];

function detectTable(doc, catalogData) {
  const text = doc.toString().toUpperCase();
  const match = text.match(/FROM\s+(\w+)/);
  if (!match) return null;
  const name = match[1].toLowerCase();
  if (catalogData.tableNames.includes(name)) return name;
  return catalogData.relationAliases[name] || null;
}

function tableCompletionOptions(catalogData) {
  const options = catalogData.tableNames.map((tableName) => ({
    label: tableName,
    type: "class",
    detail: `${(catalogData.schema[tableName] || []).length} columns`,
  }));

  for (const [alias, relationName] of Object.entries(catalogData.relationAliases)) {
    options.push({
      label: alias,
      type: "class",
      detail: `alias for ${relationName}`,
    });
  }

  return options;
}

function columnCompletionOptions(tableName, catalogData) {
  const columns = catalogData.schema[tableName] || [];
  const metadataFields = catalogData.metadataFields || [];
  const allFields = [...new Set([...columns, ...metadataFields])];
  return allFields.map((fieldName) => ({
    label: fieldName,
    type: columns.includes(fieldName) ? "property" : "variable",
    detail: columns.includes(fieldName) ? "column" : "metadata",
  }));
}

function createSyqlCompletions(catalogData) {
  const tableOptions = tableCompletionOptions(catalogData);

  return function syqlCompletions(context) {
    const before = context.state.doc.sliceString(0, context.pos);
    const word = context.matchBefore(/[\w]*/);
    if (!word && !context.explicit) return null;

    const from = word ? word.from : context.pos;

    if (/\bSCOPE\s*$/i.test(before)) {
      return {
        from,
        options: SCOPE_VALUES.map((value) => ({ label: value, type: "enum" })),
      };
    }

    if (/\bDOWNLOAD\s*$/i.test(before)) {
      return {
        from,
        options: DOWNLOAD_VALUES.map((value) => ({ label: value, type: "enum" })),
      };
    }

    if (/\bFROM\s+\w*$/i.test(before) && !/\bWHERE\b/i.test(before)) {
      return {
        from,
        options: tableOptions,
      };
    }

    const table = detectTable(context.state.doc, catalogData);
    if (table && /\b(SELECT|WHERE|AND|OR|BY)\s+[\w,\s]*$/i.test(before)) {
      return {
        from,
        options: columnCompletionOptions(table, catalogData),
      };
    }

    if (/[=<>!]+\s*\S*$/.test(before.slice(-20))) {
      if (/=\s*$/i.test(before)) {
        return {
          from,
          options: [
            { label: "TRUE", type: "keyword" },
            { label: "FALSE", type: "keyword" },
          ],
          filter: true,
        };
      }
      return null;
    }

    const options = [
      ...KEYWORDS.map((keyword) => ({ label: keyword, type: "keyword" })),
      ...CLAUSES.map((keyword) => ({ label: keyword, type: "keyword" })),
      ...tableOptions,
    ];

    if (table) {
      options.push(...columnCompletionOptions(table, catalogData));
    }

    return { from, options, filter: true };
  };
}

function createSyqlLintSource(validateUrl) {
  let lastValidatedText = "";

  return async function syqlLintSource(view) {
    const text = view.state.doc.toString().trim();
    if (!text) return [];
    if (text === lastValidatedText) return [];
    lastValidatedText = text;

    try {
      const response = await fetch(validateUrl || DEFAULT_VALIDATE_URL, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ query: text }),
      });
      const data = await response.json();
      if (data.valid) return [];
      return [
        {
          from: 0,
          to: view.state.doc.length,
          severity: "error",
          message: data.error || "Invalid query",
        },
      ];
    } catch {
      return [];
    }
  };
}

const lightTheme = EditorView.theme({
  "&": {
    fontSize: "14px",
    border: "1px solid var(--bs-border-color, #dee2e6)",
    borderRadius: "0.375rem",
  },
  ".cm-content": {
    fontFamily: "'JetBrains Mono', monospace",
    minHeight: "120px",
  },
  ".cm-gutters": { backgroundColor: "var(--bs-tertiary-bg, #f8f9fa)" },
  ".cm-focused": { outline: "2px solid var(--bs-primary, #0d6efd)" },
  ".cm-tooltip-autocomplete": {
    fontFamily: "'JetBrains Mono', monospace",
    fontSize: "13px",
  },
});

const darkThemeExt = EditorView.theme({
  "&": {
    fontSize: "14px",
    border: "1px solid var(--bs-border-color, #495057)",
    borderRadius: "0.375rem",
  },
  ".cm-content": {
    fontFamily: "'JetBrains Mono', monospace",
    minHeight: "120px",
  },
  ".cm-focused": { outline: "2px solid var(--bs-primary, #0d6efd)" },
  ".cm-tooltip-autocomplete": {
    fontFamily: "'JetBrains Mono', monospace",
    fontSize: "13px",
  },
});

const themeConf = new Compartment();

function isDarkMode() {
  return document.documentElement.getAttribute("data-theme") === "dark";
}

function getThemeExtension() {
  return isDarkMode() ? [oneDark, darkThemeExt] : [lightTheme];
}

async function initEditor(textarea) {
  if (textarea.dataset.queryfabricSyqlEditorInitialized) return;
  textarea.dataset.queryfabricSyqlEditorInitialized = "1";

  const catalogUrl = textarea.dataset.queryfabricCatalogUrl || DEFAULT_CATALOG_URL;
  const validateUrl = textarea.dataset.queryfabricValidateUrl || DEFAULT_VALIDATE_URL;
  const catalogData = await loadCatalogData(catalogUrl);
  const initialValue = textarea.value || textarea.getAttribute("value") || "";

  const wrapper = document.createElement("div");
  wrapper.className = "queryfabric-syql-cm-wrapper";
  textarea.parentNode.insertBefore(wrapper, textarea);
  textarea.style.display = "none";

  const syqlDialect = SQLDialect.define({
    keywords:
      "from select where limit and or not in like between is null true false order by asc desc scope download",
    types: [
      ...catalogData.tableNames,
      ...Object.keys(catalogData.relationAliases),
    ].join(" "),
  });

  const extensions = [
    basicSetup,
    sql({ dialect: syqlDialect }),
    autocompletion({
      override: [createSyqlCompletions(catalogData)],
      activateOnTyping: true,
      maxRenderedOptions: 30,
    }),
    lintGutter(),
    linter(createSyqlLintSource(validateUrl), { delay: 500 }),
    themeConf.of(getThemeExtension()),
    EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        textarea.value = update.state.doc.toString();
        textarea.dispatchEvent(new Event("input", { bubbles: true }));
      }
    }),
    keymap.of([
      {
        key: "Mod-Enter",
        run: () => {
          const form = textarea.closest("form");
          if (form) {
            form.requestSubmit();
          }
          return true;
        },
      },
    ]),
    EditorView.lineWrapping,
  ];

  const view = new EditorView({
    state: EditorState.create({
      doc: initialValue,
      extensions,
    }),
    parent: wrapper,
  });

  // The native textarea remains the form's successful control, including its
  // required constraint. Keep browser validation usable after the textarea is
  // replaced visually by CodeMirror.
  textarea.addEventListener("invalid", (event) => {
    event.preventDefault();
    view.focus();
  });

  const themeObserver = new MutationObserver(() => {
    view.dispatch({
      effects: themeConf.reconfigure(getThemeExtension()),
    });
  });
  themeObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-theme"],
  });

  wrapper._cmView = view;
  wrapper._cmThemeObs = themeObserver;

  requestAnimationFrame(() => view.focus());
}

let initTimer = null;

function scanForTextareas() {
  document.querySelectorAll("textarea[data-queryfabric-syql-editor]").forEach((textarea) => {
    if (!textarea.dataset.queryfabricSyqlEditorInitialized) {
      void initEditor(textarea);
    }
  });
}

const observer = new MutationObserver(() => {
  clearTimeout(initTimer);
  initTimer = setTimeout(scanForTextareas, 150);
});

observer.observe(document.documentElement, {
  childList: true,
  subtree: true,
});

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", scanForTextareas);
} else {
  scanForTextareas();
}
