const fs = require("node:fs");
const path = require("node:path");
const ts = require(path.join(process.cwd(), "node_modules/typescript"));
const ui = JSON.parse(process.argv[1]);
const paths = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
const known = new Set(paths);
const config = ts.getParsedCommandLineOfConfigFile("tsconfig.json", {}, {
    ...ts.sys,
    onUnRecoverableConfigFileDiagnostic(diagnostic) {
        throw new Error(ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n"));
    },
});
if (!config) throw new Error("Unable to read tsconfig.json");
const resolution = ts.createModuleResolutionCache(process.cwd(), f => f, config.options);
const report = Object.create(null);
for (const relative of paths) {
    const filename = path.resolve(relative);
    const text = fs.readFileSync(filename, "utf8");
    const root = ts.createSourceFile(filename, text, ts.ScriptTarget.Latest, true);
    const facts = report[relative] = { edges: Object.create(null), functions: [], findings: [], guards: [], exports: 0, code_lines: 0, comment_lines: 0, commented_code: [] };
    if (root.parseDiagnostics.length) {
        facts.parse_error = ts.flattenDiagnosticMessageText(root.parseDiagnostics[0].messageText, "\n");
        continue;
    }
    const comments = new Map();
    const code = new Set();
    const designBindings = new Set();
    const accessibility = new Set(["img", "area"]);
    const line = (offset) => root.getLineAndCharacterOfPosition(offset).line + 1;
    const finding = (id, pos, message) => facts.findings.push([id, line(pos), message]);
    function ranges(node) {
        for (const range of ts.getLeadingCommentRanges(text, node.pos) ?? []) comments.set(range.pos, range);
        for (const range of ts.getTrailingCommentRanges(text, node.end) ?? []) comments.set(range.pos, range);
    }
    function source(specifier) {
        if (!specifier || !ts.isStringLiteralLike(specifier)) return;
        const name = specifier.text;
        const found = ts.resolveModuleName(name, filename, config.options, ts.sys, resolution).resolvedModule;
        const target = found && path.relative(process.cwd(), found.resolvedFileName);
        if (known.has(target)) facts.edges[target] ??= line(specifier.getStart(root));
        else facts.edges["package:" + name] ??= line(specifier.getStart(root));
        if (ui.forbidden_imports.includes(name)) finding("S10", specifier.getStart(root), "Forbidden UI import " + name);
    }
    for (const statement of root.statements) {
        if (ts.isImportDeclaration(statement) && ts.isStringLiteralLike(statement.moduleSpecifier)
            && statement.moduleSpecifier.text === ui.component_module) {
            const imports = statement.importClause;
            if (imports?.name) designBindings.add(imports.name.text);
            if (imports?.namedBindings && ts.isNamedImports(imports.namedBindings)) {
                for (const element of imports.namedBindings.elements) designBindings.add(element.name.text);
            }
        }
        if (statement.modifiers?.some(modifier => modifier.kind === ts.SyntaxKind.ExportKeyword)) {
            facts.exports += ts.isVariableStatement(statement) ? statement.declarationList.declarations.length : 1;
        } else if (ts.isExportAssignment(statement) || ts.isExportDeclaration(statement)) {
            facts.exports += statement.exportClause && ts.isNamedExports(statement.exportClause)
                ? statement.exportClause.elements.length : 1;
        }
    }
    function syntax(node) {
        const start = node.getStart(root);
        if (node.kind === ts.SyntaxKind.AnyKeyword) finding("S3", start, "Explicit any type");
        if (ts.isNonNullExpression(node)) finding("S3", start, "Non-null assertion");
        if (ts.isAsExpression(node) && ts.isAsExpression(node.expression)
            && node.expression.type.kind === ts.SyntaxKind.UnknownKeyword) {
            finding("S3", start, "Double assertion through unknown");
        }
        if (ts.isCatchClause(node) && node.block.statements.length === 0) finding("S24", start, "Empty catch block");
        if (ts.isDebuggerStatement(node)) finding("S24", start, "Debugger statement");
        if (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) source(node.moduleSpecifier);
        if (ts.isExternalModuleReference(node)) source(node.expression);
        if (ts.isCallExpression(node) && (node.expression.kind === ts.SyntaxKind.ImportKeyword
            || (ts.isIdentifier(node.expression) && node.expression.text === "require"))) source(node.arguments[0]);
        if (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) {
            const name = node.tagName.getText(root);
            const attributes = node.attributes.properties.filter(ts.isJsxAttribute);
            const spread = node.attributes.properties.some(ts.isJsxSpreadAttribute);
            if (ui.forbidden_elements.includes(name)) finding("S10", start, "Forbidden JSX element " + name);
            if (designBindings.has(name)) {
                for (const attribute of attributes) {
                    const prop = attribute.name.getText(root);
                    if (ui.forbidden_props.includes(prop)) {
                        finding("S10", attribute.getStart(root), "Forbidden prop on " + name + ": " + prop);
                    }
                }
            }
            if (accessibility.has(name) && !spread && !attributes.some(attr => attr.name.getText(root) === "alt")) {
                finding("S25", start, "Missing alt attribute on " + name);
            }
        }
        if (ts.isFunctionLike(node) && node.body) {
            facts.functions.push({
                name: node.name?.getText(root) ?? (ts.isVariableDeclaration(node.parent) ? node.parent.name.getText(root) : "anonymous function"),
                line: line(start),
                lines: line(node.end) - line(start) + 1,
                tokens: leaves(node.body),
            });
        }
        ts.forEachChild(node, syntax);
    }
    function walkLeaves(node, visit, includeComments) {
        if (includeComments) ranges(node);
        if (node.kind === ts.SyntaxKind.JSDocComment) return;
        const children = node.getChildren(root);
        if (!children.length && node.end > node.getStart(root)) visit(node);
        else for (const child of children) walkLeaves(child, visit, includeComments);
    }
    function leaves(node) {
        const tokens = [];
        walkLeaves(node, leaf => tokens.push(leaf.getText(root)), false);
        return tokens;
    }
    syntax(root);
    walkLeaves(root, leaf => {
        for (let i = line(leaf.getStart(root)); i <= line(leaf.end - 1); i++) code.add(i);
    }, true);
    facts.code_lines = code.size;
    const commentLines = new Set();
    for (const span of comments.values()) {
        const body = text.slice(span.pos, span.end);
        for (const [offset, row] of body.split(/\r?\n/).entries()) {
            const n = line(span.pos) + offset;
            commentLines.add(n);
            if (/\b(TODO|FIXME|HACK|XXX)\b/.test(row)) facts.findings.push(["S13", n, "Maintenance marker"]);
            if (/in a real implementation/i.test(row)) facts.findings.push(["S24", n, "Placeholder comment"]);
            if (/^\s*[/\s*]*@ts-(ignore|expect-error|nocheck)\b/.test(row)) {
                facts.findings.push(["S3", n, "TypeScript suppression directive"]);
                facts.guards.push(row.trim());
            }
            if (/^\s*[/\s*]*(eslint-disable(?:-next-line|-line)?|biome-ignore|oxlint-disable|prettier-ignore)\b/.test(row)) {
                facts.findings.push(["S5", n, row.trim()]);
                facts.guards.push(row.trim());
            }
            if (/^\s*[/\s*]*(c8|istanbul|v8) ignore\b|^\s*[/\s*]*node:coverage\s+(disable|ignore)\b/.test(row)) facts.guards.push(row.trim());
            if (/^\s*[/\s*]*@deprecated\b/.test(row)) facts.findings.push(["S13", n, "Deprecation marker"]);
            const statement = row.replace(/^[/\s*]+/, "").trimEnd();
            if (/^(let|const|var)\s/.test(statement) && statement.endsWith(";")) {
                const candidate = ts.createSourceFile("comment.ts", statement, ts.ScriptTarget.Latest, false);
                if (!candidate.parseDiagnostics.length && candidate.statements.length === 1
                    && ts.isVariableStatement(candidate.statements[0])) facts.commented_code.push(n);
            }
        }
    }
    facts.comment_lines = commentLines.size;
}
process.stdout.write(JSON.stringify(report));
