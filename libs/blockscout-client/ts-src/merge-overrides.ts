import * as fs from 'fs';
import * as path from 'path';
import { parse, stringify } from 'yaml';
import { merge } from 'lodash';

const OVERRIDES_DIR = path.join(__dirname, '..', 'swaggers', 'overrides');
const MAIN_SPEC_PATH = path.join(__dirname, '..', 'swaggers', 'blockscout-api-final.yaml');

async function mergeOverrides() {
    try {
        // Read the main spec file
        const mainSpecContent = fs.readFileSync(MAIN_SPEC_PATH, 'utf8');
        let mergedSpec = parse(mainSpecContent);

        // Read all YAML files from the overrides directory
        const overrideFiles = fs.readdirSync(OVERRIDES_DIR)
            .filter(file => file.endsWith('.yaml'));

        // Sort files to ensure consistent merging order
        overrideFiles.sort();

        // Apply each override
        for (const file of overrideFiles) {
            const overridePath = path.join(OVERRIDES_DIR, file);
            const overrideContent = fs.readFileSync(overridePath, 'utf8');
            const overrideSpec = parse(overrideContent);

            // Deep merge the override with the main spec
            mergedSpec = merge(mergedSpec, overrideSpec);
        }

        // Write the merged result back to the main spec file
        const mergedYaml = stringify(mergedSpec);
        fs.writeFileSync(MAIN_SPEC_PATH, mergedYaml);

        console.log(`Successfully merged all overrides into ${MAIN_SPEC_PATH}`);
    } catch (error) {
        console.error('Error merging overrides:', error);
        process.exit(1);
    }
}

mergeOverrides(); 