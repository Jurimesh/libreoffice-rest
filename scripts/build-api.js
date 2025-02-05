const esbuild = require("esbuild");
const path = require("path");

const EXTERNALS = [];

async function run() {
  await esbuild.build({
    outfile: path.join(__dirname, "../dist/bundle.prod.js"),
    write: true,
    bundle: true,
    minify: true,
    sourcemap: true,
    platform: "node",
    format: "cjs",
    jsx: "transform",
    entryPoints: [path.join(__dirname, "../src/entry.ts")],
    external: EXTERNALS,
    plugins: [],
  });
}

run()
  .then(() => {
    console.log("Finished bundling api");
  })
  .catch((err) => {
    console.error("Could not bundle api");
    console.error(err);
    process.exit(1);
  });
