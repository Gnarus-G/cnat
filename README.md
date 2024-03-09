## Why?

Because you joined a project that has some awful old tailwind configs: weird bespoke spacing configuration,
weird sizes, some current tailwind default colors (e.g. slate) not available because the config was for an older version
of tailwind, etc...

So now you can't just copy/paste tailwind class names from the internet or use ones that come from a ui component library.
Say you want to use [shadcn/ui](https://ui.shadcn.com/); Nope!

You want to do something about this, but it's way too risky and time consuming. You would want to do this incrementally, to protect your
sanity.

The best solution would be to deprecate the old configs while keeping them around and working; so you slap a prefix `legacy-`
in the old `tailwind.config.js`.

## Deprecation Steps

In the root of your project. Run:

```sh
npx tailwindcss -i <(echo '@tailwind utilities;') -o legacy-tw.css
cnat prefix -i legacy-tw.css --prefix 'legacy-' .
```

By default, `cnat prefix` will crawl through all the `class=*`, `className=*` in jsx elements and `className:*` in a `React.createElement` calls, inside of `ts|js|tsx|jsx` files.
It will match any class in the source code with classes found in `legacy-tw.css` (which contains every style that tailwind generates based on your config).

Add the prefix in the legacy config file.

```js
/** @type {import('tailwindcss').Config} */
module.exports = {
  prefix: "legacy-",
};
```

Then update your global css with the tailwind directives. Create a new file for the old configs, `tailwind.legacy.config.js`.
Then, create an additional css file which the tailwind directives as such:

`./legacy-tw.css`

```css
@config "./tailwind.legacy.config.js";
@tailwind base;
@tailwind components;
@tailwind utilities;
```

`@config` requires tailwind v3.2 btw.

Import that css file in where-ever the entry point for your project is. For example in Nextjs, you can add it to the `_app.tsx` file (pages directory version).

```ts
import "./legacy-tw.css";
```

## Install

```sh
cargo install cnat
```

```sh
npm install -g cnat
```

Or just execute it npm:

```sh
npx cnat
```

## Usage

```
Usage: cnat <COMMAND>

Commands:
  prefix  Apply a prefix to all the tailwind classes in every js file in a project
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

```sh
cnat prefix --help
```

```
Apply a prefix to all the tailwind classes in every js file in a project

Usage: cnat prefix [OPTIONS] -i <CSS_FILE> --prefix <PREFIX> <CONTEXT>

Arguments:
  <CONTEXT>  The root directory of the js/ts project

Options:
  -i <CSS_FILE>             The output css file generated by calling `npx tailwindcss -i input.css -o output.css`
  -p, --prefix <PREFIX>     The prefix to apply to all the tailwind class names found
  -s, --scopes <SCOPES>...  Define scope within which prefixing happens. Example: --scopes 'att:className,*ClassName prop:classes fn:cva' [default: "att:class,className fn:createElement"]
  -h, --help                Print help
```
