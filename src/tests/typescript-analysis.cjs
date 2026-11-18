const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { test } = require("node:test");

const parser = process.env.LATHER_TYPESCRIPT_MODULE;
const analyzer = fs.readFileSync(path.join(__dirname, "../hygiene/typescript.cjs"), "utf8");

test("reports tracked TS, TSX, and JS syntax and resolves internal imports", () => {
    assert.ok(parser, "Set LATHER_TYPESCRIPT_MODULE to the project's pinned node_modules/typescript");
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "lather-ast-"));
    try {
        fs.mkdirSync(path.join(root, "node_modules"));
        fs.symlinkSync(path.resolve(parser), path.join(root, "node_modules/typescript"), "dir");
        const files = {
            "a.tsx": `import { Button as Primary } from "@example/ui";
export { b } from "@local/b";
// TODO known issue
// let stale = true;
const literal = "eslint-disable TODO in a real implementation";
const v = ({} as unknown as { good: boolean }).good!;
const explicit: any = v;
// @ts-expect-error only for fixture
export const component = <><button /><Primary className="bad" /><img /></>;
/* istanbul ignore next */
export function blank() { try { return literal; } catch {} }
`,
            "b.ts": "export { blank as b } from './a';\n",
            "c.js": "const x = require('./b');\n// eslint-disable-next-line no-debugger\ndebugger;\n",
            "d.test.js": `/* eslint-disable */
const literal = "/* oxlint-disable */";
// oxlint-disable-next-line landmines-testing/no-fixed-sleep
debugger;
`,
        };
        for (const [file, body] of Object.entries(files)) fs.writeFileSync(path.join(root, file), body);
        fs.writeFileSync(path.join(root, "paths.json"), JSON.stringify({
            production: ["a.tsx", "b.ts", "c.js"],
            tests: ["d.test.js"],
        }));
        fs.writeFileSync(path.join(root, "tsconfig.json"), JSON.stringify({
            compilerOptions: { allowJs: true, jsx: "preserve", paths: { "@local/*": ["./*"] } },
        }));
        const ui = {
            forbidden_elements: ["button"],
            forbidden_imports: [],
            component_module: "@example/ui",
            forbidden_props: ["className"],
        };
        const data = JSON.parse(execFileSync(process.execPath, ["-e", analyzer, JSON.stringify(ui), "paths.json"], { cwd: root }));
        const hits = id => data["a.tsx"].findings.filter(finding => finding[0] === id).map(finding => finding[1]);
        assert.deepEqual(data["a.tsx"].edges, { "b.ts": 2, "package:@example/ui": 1 });
        assert.deepEqual(data["b.ts"].edges, { "a.tsx": 1 });
        assert.deepEqual(data["c.js"].edges, { "b.ts": 1 });
        assert.deepEqual(hits("S3").sort((a, b) => a - b), [6, 6, 7, 8]);
        assert.deepEqual(hits("S10"), [9, 9]);
        assert.deepEqual(hits("S13"), [3]);
        assert.deepEqual(hits("S24"), [11]);
        assert.deepEqual(hits("S25"), [9]);
        assert.deepEqual(data["a.tsx"].commented_code, [4]);
        assert.deepEqual(data["a.tsx"].guards, ["// @ts-expect-error only for fixture", "/* istanbul ignore next */"]);
        assert.deepEqual(data["c.js"].findings.map(finding => finding.slice(0, 2)).sort(), [["S24", 3], ["S5", 2]]);
        assert.deepEqual(data["d.test.js"].guards, ["eslint-disable */", "oxlint-disable-next-line landmines-testing/no-fixed-sleep"]);
        assert.deepEqual(data["d.test.js"].findings, []);
    } finally {
        fs.rmSync(root, { recursive: true, force: true });
    }
});
