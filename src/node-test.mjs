import { run } from "node:test";

for await (const event of run({ files: process.argv.slice(2), execArgv: [] })) {
  if (event.type === "test:pass" || event.type === "test:fail") {
    const { file, name, details, skip, todo } = event.data;
    if (details?.type === "test" && !skip && !todo) {
      console.log(JSON.stringify({ file, name }));
    }
  } else if (event.type === "test:summary" && !event.data.file) {
    console.log(JSON.stringify({ counts: event.data.counts }));
    process.exitCode = event.data.success ? 0 : 1;
  }
}
