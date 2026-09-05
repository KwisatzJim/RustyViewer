import fs from "node:fs";

const cargo = fs.readFileSync("Cargo.toml", "utf8");
const cargoVersion = cargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
const tauriVersion = JSON.parse(fs.readFileSync("tauri.conf.json", "utf8")).version;
const packageVersion = JSON.parse(fs.readFileSync("package.json", "utf8")).version;
const lockVersion = JSON.parse(fs.readFileSync("package-lock.json", "utf8")).version;
const versions = { Cargo: cargoVersion, Tauri: tauriVersion, package: packageVersion, lockfile: lockVersion };

if (!cargoVersion || new Set(Object.values(versions)).size !== 1) {
  console.error("Release versions do not match:", versions);
  process.exit(1);
}

const tagIndex = process.argv.indexOf("--tag");
const tag = tagIndex !== -1 ? process.argv[tagIndex + 1] : process.env.RELEASE_TAG;
if (tag) {
  if (tag !== cargoVersion) {
    console.error(`Git tag ${tag} does not match application version ${cargoVersion}.`);
    process.exit(1);
  }
}

console.log(`RustyViewer version ${cargoVersion} is consistent.`);
