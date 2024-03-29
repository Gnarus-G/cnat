[![crates.io](https://img.shields.io/crates/v/cnat.svg)](https://crates.io/crates/cnat)
[![npm version](https://img.shields.io/npm/v/cnat.svg)](https://www.npmjs.com/package/cnat)

# CNAT

Class Name Alteration Tool. Systematically change all the class names in your codebase.

## Install

```sh
cargo install cnat
```

```sh
npm install -g cnat
```

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
echo '@tailwind base; @tailwind components; @tailwind utilities;' > temp.css
npx tailwindcss -i temp.css -o legacy-tw.css
cnat prefix -i legacy-tw.css --prefix 'legacy-' ./src
```

By default, `cnat prefix` will crawl through all the `class=*`, `className=*` in jsx elements and `className:*` in a `React.createElement` calls, inside of `ts|js|tsx|jsx` files.
It will match any class in the source code with classes found in `legacy-tw.css` (which contains every style that tailwind generates based on your config).

Rename the file for the old configs, `tailwind.legacy.config.ts`.

```sh
mv tailwind.config.ts tailwind.legacy.config.ts # or *.js if your tailwind config files aren't in typescript
```

Add the prefix in the legacy config file.

```ts
export default {
  prefix: "legacy-",
};
```

Run the tailwind cli again to rebuild the legacy css classes. Now they will be prefixed since the `tailwind.legacy.config.ts` defined
the 'legacy-'

```sh
npx tailwindcss -i temp.css -o legacy-tw.css -c tailwind.legacy.config.ts
```

Then, import the legacy css into your global css file.

```css
@import "./legacy-tw.css";

@tailwind base;
@tailwind components;
@tailwind utilities;

/* And whatever else you have in here */
```

Now you can re-init new configs.

```sh
# You'd have to delete the current `tailwind.config.ts` if you haven't already.
npx tailwindcss init --ts
```

And now you can breathe. A fresh new start; ignoring what's under the bed now.

## Usage

```
Systematically apply certain modifications to classes, class names, used in your frontend codebase.

Usage: cnat <COMMAND>

Commands:
  prefix      Apply a prefix to all the tailwind classes in every js file in a project
  completion  Generate completions for a specified shell
  help        Print this message or the help of the given subcommand(s)

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

### Scopes

You may have tailwind classes in other places besides `className="..."`, or even `cva(...)`.
For examples, the `classes` prop in mui components.

You can define places for `cnat` to look for classes with `--scopes` or `-s` option.
The syntax for a scope is <variant>:<...values>

**Variants** are:

- `fn` to target a function call (e.g 'fn:cva')
- `att` to target a jsx attribute (e.g. 'att:className')
- `prop` to target a jsx attribute (e.g. 'prop:className')

**Values** are strings, and you can use a wildcard `*` at the begining or the end.
For example 'att:className att:\*ClassName' will find classes all of these attributes

```js
<Btn
  className="w-10 bg-red"
  iconClassName="text-black"
  textClassName="text-xl"
/>
```

By default `cnat` use --scopes 'att:class,className fn:createElement'

```sh
cnat prefix -i legacy-tw.css --prefix 'legacy-' ./src --scopes 'att:class,className fn:createElement'
```
