#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bundle_dir="${1:-${repo_root}/docker/distributable}"
archive_path="${2:-${repo_root}/artifacts/attune-docker-dist.tar.gz}"

template_dir="${repo_root}/docker/distributable"
bundle_dir="$(realpath -m "${bundle_dir}")"
archive_path="$(realpath -m "${archive_path}")"
template_dir="$(realpath -m "${template_dir}")"

mkdir -p "${bundle_dir}/docker" "${bundle_dir}/migrations" "${bundle_dir}/packs" "${bundle_dir}/scripts"
mkdir -p "$(dirname "${archive_path}")"

copy_file() {
    local src="$1"
    local dst="$2"
    mkdir -p "$(dirname "${dst}")"
    cp "${src}" "${dst}"
}

# Keep the distributable compose file, README, and config as the maintained templates.
if [ "${bundle_dir}" != "${template_dir}" ]; then
    copy_file "${template_dir}/docker-compose.yaml" "${bundle_dir}/docker-compose.yaml"
    copy_file "${template_dir}/README.md" "${bundle_dir}/README.md"
    copy_file "${template_dir}/config.docker.yaml" "${bundle_dir}/config.docker.yaml"
fi

copy_file "${repo_root}/docker/run-migrations.sh" "${bundle_dir}/docker/run-migrations.sh"
copy_file "${repo_root}/docker/init-user.sh" "${bundle_dir}/docker/init-user.sh"
copy_file "${repo_root}/docker/init-packs.sh" "${bundle_dir}/docker/init-packs.sh"
copy_file "${repo_root}/docker/init-roles.sql" "${bundle_dir}/docker/init-roles.sql"
copy_file "${repo_root}/docker/nginx.conf" "${bundle_dir}/docker/nginx.conf"
copy_file "${repo_root}/docker/inject-env.sh" "${bundle_dir}/docker/inject-env.sh"
copy_file "${repo_root}/scripts/load_core_pack.py" "${bundle_dir}/scripts/load_core_pack.py"

rm -rf "${bundle_dir}/migrations" "${bundle_dir}/packs/core"
mkdir -p "${bundle_dir}/migrations" "${bundle_dir}/packs"
cp -R "${repo_root}/migrations/." "${bundle_dir}/migrations/"
cp -R "${repo_root}/packs/core" "${bundle_dir}/packs/core"

tar -C "$(dirname "${bundle_dir}")" -czf "${archive_path}" "$(basename "${bundle_dir}")"

echo "Docker dist bundle refreshed at ${bundle_dir}"
echo "Docker dist archive created at ${archive_path}"
