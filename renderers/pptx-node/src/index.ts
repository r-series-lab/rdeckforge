import { readFileSync } from "node:fs";
import { renderPptx } from "./render-pptx.js";

type CliArgs = {
  job?: string;
};

function parseArgs(argv: string[]): CliArgs {
  const args: CliArgs = {};
  for (let index = 0; index < argv.length; index += 1) {
    const item = argv[index];
    if (item === "--job") {
      args.job = argv[index + 1];
      index += 1;
    }
  }
  return args;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (!args.job) {
    console.log(
      JSON.stringify({
        ok: false,
        error: {
          code: "invalid_arguments",
          message: "--job is required",
        },
      }),
    );
    process.exitCode = 2;
    return;
  }

  const job = JSON.parse(readFileSync(args.job, "utf8"));
  try {
    const data = await renderPptx(job);
    console.log(JSON.stringify({ ok: true, data }));
  } catch (error) {
    console.log(
      JSON.stringify({
        ok: false,
        error: {
          code: "render_failed",
          message: error instanceof Error ? error.message : String(error),
        },
      }),
    );
    process.exitCode = 1;
  }
}

main();
