
// this file is generated — do not edit it


/// <reference types="@sveltejs/kit" />

/**
 * This module provides access to environment variables that are injected _statically_ into your bundle at build time and are limited to _private_ access.
 * 
 * |         | Runtime                                                                    | Build time                                                               |
 * | ------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
 * | Private | [`$env/dynamic/private`](https://svelte.dev/docs/kit/$env-dynamic-private) | [`$env/static/private`](https://svelte.dev/docs/kit/$env-static-private) |
 * | Public  | [`$env/dynamic/public`](https://svelte.dev/docs/kit/$env-dynamic-public)   | [`$env/static/public`](https://svelte.dev/docs/kit/$env-static-public)   |
 * 
 * Static environment variables are [loaded by Vite](https://vitejs.dev/guide/env-and-mode.html#env-files) from `.env` files and `process.env` at build time and then statically injected into your bundle at build time, enabling optimisations like dead code elimination.
 * 
 * **_Private_ access:**
 * 
 * - This module cannot be imported into client-side code
 * - This module only includes variables that _do not_ begin with [`config.kit.env.publicPrefix`](https://svelte.dev/docs/kit/configuration#env) _and do_ start with [`config.kit.env.privatePrefix`](https://svelte.dev/docs/kit/configuration#env) (if configured)
 * 
 * For example, given the following build time environment:
 * 
 * ```env
 * ENVIRONMENT=production
 * PUBLIC_BASE_URL=http://site.com
 * ```
 * 
 * With the default `publicPrefix` and `privatePrefix`:
 * 
 * ```ts
 * import { ENVIRONMENT, PUBLIC_BASE_URL } from '$env/static/private';
 * 
 * console.log(ENVIRONMENT); // => "production"
 * console.log(PUBLIC_BASE_URL); // => throws error during build
 * ```
 * 
 * The above values will be the same _even if_ different values for `ENVIRONMENT` or `PUBLIC_BASE_URL` are set at runtime, as they are statically replaced in your code with their build time values.
 */
declare module '$env/static/private' {
	export const CLAUDE_AUTOCOMPACT_PCT_OVERRIDE: string;
	export const CLAUDE_CODE_MESSAGING_SOCKET: string;
	export const INFOPATH: string;
	export const HOMEBREW_CELLAR: string;
	export const CLAUDE_CODE_SESSION_ID: string;
	export const npm_config_user_agent: string;
	export const COREPACK_ENABLE_AUTO_PIN: string;
	export const SSH_CONNECTION: string;
	export const npm_lifecycle_script: string;
	export const CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC: string;
	export const NODE_ENV: string;
	export const CLAUDE_CODE_EXECPATH: string;
	export const LOGNAME: string;
	export const ANTHROPIC_BASE_URL: string;
	export const SHLVL: string;
	export const STARSHIP_SESSION_KEY: string;
	export const npm_package_json: string;
	export const CLAUDE_CODE_SESSION_ATTENDED: string;
	export const npm_package_name: string;
	export const PWD: string;
	export const npm_config_node_gyp: string;
	export const CLAUDE_CODE_ATTRIBUTION_HEADER: string;
	export const TMUX_PANE: string;
	export const npm_command: string;
	export const CLAUDECODE: string;
	export const TERM_PROGRAM_VERSION: string;
	export const TMUX: string;
	export const CLAUDE_CODE_AUTO_COMPACT_WINDOW: string;
	export const TAURI_ENV_FAMILY: string;
	export const PATH: string;
	export const pnpm_config_verify_deps_before_run: string;
	export const TAURI_ENV_PLATFORM_VERSION: string;
	export const __CF_USER_TEXT_ENCODING: string;
	export const mount_authenticator_shm: string;
	export const TAURI_ENV_ARCH: string;
	export const SVELTEKIT_FORK: string;
	export const npm_node_execpath: string;
	export const PNPM_SCRIPT_SRC_DIR: string;
	export const npm_package_version: string;
	export const ANTHROPIC_MODEL: string;
	export const USER: string;
	export const GIT_EDITOR: string;
	export const LANG: string;
	export const CLAUDE_CODE_MESSAGING_TOKEN: string;
	export const CLAUDE_CODE_ENTRYPOINT: string;
	export const CLAUDE_CODE_CHILD_SESSION: string;
	export const SSH_CLIENT: string;
	export const SHELL: string;
	export const TAURI_ENV_TARGET_TRIPLE: string;
	export const npm_lifecycle_event: string;
	export const FPATH: string;
	export const AI_AGENT: string;
	export const SSH_TTY: string;
	export const TERM: string;
	export const TMPDIR: string;
	export const COLORTERM: string;
	export const NODE_PATH: string;
	export const TAURI_ENV_PLATFORM: string;
	export const CLAUDE_PID: string;
	export const NODE: string;
	export const npm_execpath: string;
	export const ANTHROPIC_AUTH_TOKEN: string;
	export const TAURI_CLI_VERBOSITY: string;
	export const HOMEBREW_PREFIX: string;
	export const HOMEBREW_REPOSITORY: string;
	export const HOME: string;
	export const TERM_PROGRAM: string;
	export const STARSHIP_SHELL: string;
	export const NoDefaultCurrentDirectoryInExePath: string;
	export const INIT_CWD: string;
	export const CLAUDE_EFFORT: string;
}

/**
 * This module provides access to environment variables that are injected _statically_ into your bundle at build time and are _publicly_ accessible.
 * 
 * |         | Runtime                                                                    | Build time                                                               |
 * | ------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
 * | Private | [`$env/dynamic/private`](https://svelte.dev/docs/kit/$env-dynamic-private) | [`$env/static/private`](https://svelte.dev/docs/kit/$env-static-private) |
 * | Public  | [`$env/dynamic/public`](https://svelte.dev/docs/kit/$env-dynamic-public)   | [`$env/static/public`](https://svelte.dev/docs/kit/$env-static-public)   |
 * 
 * Static environment variables are [loaded by Vite](https://vitejs.dev/guide/env-and-mode.html#env-files) from `.env` files and `process.env` at build time and then statically injected into your bundle at build time, enabling optimisations like dead code elimination.
 * 
 * **_Public_ access:**
 * 
 * - This module _can_ be imported into client-side code
 * - **Only** variables that begin with [`config.kit.env.publicPrefix`](https://svelte.dev/docs/kit/configuration#env) (which defaults to `PUBLIC_`) are included
 * 
 * For example, given the following build time environment:
 * 
 * ```env
 * ENVIRONMENT=production
 * PUBLIC_BASE_URL=http://site.com
 * ```
 * 
 * With the default `publicPrefix` and `privatePrefix`:
 * 
 * ```ts
 * import { ENVIRONMENT, PUBLIC_BASE_URL } from '$env/static/public';
 * 
 * console.log(ENVIRONMENT); // => throws error during build
 * console.log(PUBLIC_BASE_URL); // => "http://site.com"
 * ```
 * 
 * The above values will be the same _even if_ different values for `ENVIRONMENT` or `PUBLIC_BASE_URL` are set at runtime, as they are statically replaced in your code with their build time values.
 */
declare module '$env/static/public' {
	
}

/**
 * This module provides access to environment variables set _dynamically_ at runtime and that are limited to _private_ access.
 * 
 * |         | Runtime                                                                    | Build time                                                               |
 * | ------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
 * | Private | [`$env/dynamic/private`](https://svelte.dev/docs/kit/$env-dynamic-private) | [`$env/static/private`](https://svelte.dev/docs/kit/$env-static-private) |
 * | Public  | [`$env/dynamic/public`](https://svelte.dev/docs/kit/$env-dynamic-public)   | [`$env/static/public`](https://svelte.dev/docs/kit/$env-static-public)   |
 * 
 * Dynamic environment variables are defined by the platform you're running on. For example if you're using [`adapter-node`](https://github.com/sveltejs/kit/tree/main/packages/adapter-node) (or running [`vite preview`](https://svelte.dev/docs/kit/cli)), this is equivalent to `process.env`.
 * 
 * **_Private_ access:**
 * 
 * - This module cannot be imported into client-side code
 * - This module includes variables that _do not_ begin with [`config.kit.env.publicPrefix`](https://svelte.dev/docs/kit/configuration#env) _and do_ start with [`config.kit.env.privatePrefix`](https://svelte.dev/docs/kit/configuration#env) (if configured)
 * 
 * > [!NOTE] In `dev`, `$env/dynamic` includes environment variables from `.env`. In `prod`, this behavior will depend on your adapter.
 * 
 * > [!NOTE] To get correct types, environment variables referenced in your code should be declared (for example in an `.env` file), even if they don't have a value until the app is deployed:
 * >
 * > ```env
 * > MY_FEATURE_FLAG=
 * > ```
 * >
 * > You can override `.env` values from the command line like so:
 * >
 * > ```sh
 * > MY_FEATURE_FLAG="enabled" npm run dev
 * > ```
 * 
 * For example, given the following runtime environment:
 * 
 * ```env
 * ENVIRONMENT=production
 * PUBLIC_BASE_URL=http://site.com
 * ```
 * 
 * With the default `publicPrefix` and `privatePrefix`:
 * 
 * ```ts
 * import { env } from '$env/dynamic/private';
 * 
 * console.log(env.ENVIRONMENT); // => "production"
 * console.log(env.PUBLIC_BASE_URL); // => undefined
 * ```
 */
declare module '$env/dynamic/private' {
	export const env: {
		CLAUDE_AUTOCOMPACT_PCT_OVERRIDE: string;
		CLAUDE_CODE_MESSAGING_SOCKET: string;
		INFOPATH: string;
		HOMEBREW_CELLAR: string;
		CLAUDE_CODE_SESSION_ID: string;
		npm_config_user_agent: string;
		COREPACK_ENABLE_AUTO_PIN: string;
		SSH_CONNECTION: string;
		npm_lifecycle_script: string;
		CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC: string;
		NODE_ENV: string;
		CLAUDE_CODE_EXECPATH: string;
		LOGNAME: string;
		ANTHROPIC_BASE_URL: string;
		SHLVL: string;
		STARSHIP_SESSION_KEY: string;
		npm_package_json: string;
		CLAUDE_CODE_SESSION_ATTENDED: string;
		npm_package_name: string;
		PWD: string;
		npm_config_node_gyp: string;
		CLAUDE_CODE_ATTRIBUTION_HEADER: string;
		TMUX_PANE: string;
		npm_command: string;
		CLAUDECODE: string;
		TERM_PROGRAM_VERSION: string;
		TMUX: string;
		CLAUDE_CODE_AUTO_COMPACT_WINDOW: string;
		TAURI_ENV_FAMILY: string;
		PATH: string;
		pnpm_config_verify_deps_before_run: string;
		TAURI_ENV_PLATFORM_VERSION: string;
		__CF_USER_TEXT_ENCODING: string;
		mount_authenticator_shm: string;
		TAURI_ENV_ARCH: string;
		SVELTEKIT_FORK: string;
		npm_node_execpath: string;
		PNPM_SCRIPT_SRC_DIR: string;
		npm_package_version: string;
		ANTHROPIC_MODEL: string;
		USER: string;
		GIT_EDITOR: string;
		LANG: string;
		CLAUDE_CODE_MESSAGING_TOKEN: string;
		CLAUDE_CODE_ENTRYPOINT: string;
		CLAUDE_CODE_CHILD_SESSION: string;
		SSH_CLIENT: string;
		SHELL: string;
		TAURI_ENV_TARGET_TRIPLE: string;
		npm_lifecycle_event: string;
		FPATH: string;
		AI_AGENT: string;
		SSH_TTY: string;
		TERM: string;
		TMPDIR: string;
		COLORTERM: string;
		NODE_PATH: string;
		TAURI_ENV_PLATFORM: string;
		CLAUDE_PID: string;
		NODE: string;
		npm_execpath: string;
		ANTHROPIC_AUTH_TOKEN: string;
		TAURI_CLI_VERBOSITY: string;
		HOMEBREW_PREFIX: string;
		HOMEBREW_REPOSITORY: string;
		HOME: string;
		TERM_PROGRAM: string;
		STARSHIP_SHELL: string;
		NoDefaultCurrentDirectoryInExePath: string;
		INIT_CWD: string;
		CLAUDE_EFFORT: string;
		[key: `PUBLIC_${string}`]: undefined;
		[key: `${string}`]: string | undefined;
	}
}

/**
 * This module provides access to environment variables set _dynamically_ at runtime and that are _publicly_ accessible.
 * 
 * |         | Runtime                                                                    | Build time                                                               |
 * | ------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
 * | Private | [`$env/dynamic/private`](https://svelte.dev/docs/kit/$env-dynamic-private) | [`$env/static/private`](https://svelte.dev/docs/kit/$env-static-private) |
 * | Public  | [`$env/dynamic/public`](https://svelte.dev/docs/kit/$env-dynamic-public)   | [`$env/static/public`](https://svelte.dev/docs/kit/$env-static-public)   |
 * 
 * Dynamic environment variables are defined by the platform you're running on. For example if you're using [`adapter-node`](https://github.com/sveltejs/kit/tree/main/packages/adapter-node) (or running [`vite preview`](https://svelte.dev/docs/kit/cli)), this is equivalent to `process.env`.
 * 
 * **_Public_ access:**
 * 
 * - This module _can_ be imported into client-side code
 * - **Only** variables that begin with [`config.kit.env.publicPrefix`](https://svelte.dev/docs/kit/configuration#env) (which defaults to `PUBLIC_`) are included
 * 
 * > [!NOTE] In `dev`, `$env/dynamic` includes environment variables from `.env`. In `prod`, this behavior will depend on your adapter.
 * 
 * > [!NOTE] To get correct types, environment variables referenced in your code should be declared (for example in an `.env` file), even if they don't have a value until the app is deployed:
 * >
 * > ```env
 * > MY_FEATURE_FLAG=
 * > ```
 * >
 * > You can override `.env` values from the command line like so:
 * >
 * > ```sh
 * > MY_FEATURE_FLAG="enabled" npm run dev
 * > ```
 * 
 * For example, given the following runtime environment:
 * 
 * ```env
 * ENVIRONMENT=production
 * PUBLIC_BASE_URL=http://example.com
 * ```
 * 
 * With the default `publicPrefix` and `privatePrefix`:
 * 
 * ```ts
 * import { env } from '$env/dynamic/public';
 * console.log(env.ENVIRONMENT); // => undefined, not public
 * console.log(env.PUBLIC_BASE_URL); // => "http://example.com"
 * ```
 * 
 * ```
 * 
 * ```
 */
declare module '$env/dynamic/public' {
	export const env: {
		[key: `PUBLIC_${string}`]: string | undefined;
	}
}
