const path = require("path");

const webpack = require("webpack");
const HtmlWebpackPlugin = require("html-webpack-plugin");
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");

const config = (env, argv) => {
    const is_dev_server = env.WEBPACK_SERVE;
    const pkg = path.resolve(__dirname, "pkg");
    const dist = path.resolve(__dirname, "dist");
    return {
        name: "seedapp",
        performance: {
            // Don't break compilation because of WASM file bigger than 244 KB.
            hints: false
        },
        mode: "development",
        entry: {
            // Bundle root with name `app.js`.
            app: path.resolve(__dirname, "index.ts")
        },
        output: {
            // You can change it, but then also edit `historyApiFallback` below
            // and `getWebviewContent()` in `vscode-extension/src/extension.ts`
            publicPath: is_dev_server ? "http://127.0.0.1:8888/" : "/",
            // You can deploy your site from this folder (after build with e.g. `yarn build:release`)
            path: dist,
            filename: '[name].[contenthash].js',
            clean: true,
        },
        devServer: {
            host: "127.0.0.1",
            port: 8888,
            // Probably not needed, but if we ever need it, it should be the
            // same as publicPath:
            historyApiFallback: {
                index: is_dev_server ? "http://127.0.0.1:8888/" : "/",
            },
            hot: true,
            liveReload: false,
            headers: {
                "Access-Control-Allow-Origin": "*",
                "Access-Control-Allow-Methods": "GET, POST, PUT, DELETE, PATCH, OPTIONS",
                "Access-Control-Allow-Headers": "X-Requested-With, content-type, Authorization"
            },
        },
        plugins: [
            // Add scripts, css, ... to html template.
            new HtmlWebpackPlugin({
                title: "Test page",
                template: path.resolve(__dirname, "./index.html"),
            }),
            new webpack.HotModuleReplacementPlugin({}),
            // Compile Rust.1
            new WasmPackPlugin({
                crateDirectory: __dirname,

                // Optional space delimited arguments to appear before the wasm-pack
                // command. Default arguments are `--verbose`.
                args: "--log-level warn",
                extraArgs: "--target web",

                outName: "seedapp_rust",
                outDir: pkg,
            }),
        ],
        // Webpack try to guess how to resolve imports in this order:
        resolve: {
            extensions: [".ts", ".js", ".wasm"],
            alias: {
                crate: pkg,
            }
        },
        module: {
            rules: [
                {
                    test: /\.(jpg|jpeg|png|woff|woff2|eot|ttf|svg)$/,
                    loader: "file-loader"
                },
                {
                    test: /\.ts$/,
                    loader: "ts-loader",
                    options: {
                        configFile: "tsconfig.json",
                    }
                }
            ]
        },

        experiments: {
            // this adds ~2MIN ADDITIONAL COMPILATION TIME!
            // after removing `asyncWebAssembly: true` build time decreased
            // from 2min to 7s!
            // Related:
            // https://github.com/rustwasm/wasm-pack/issues/790
            // https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Statements/import#dynamic_imports
            // https://rustwasm.github.io/docs/wasm-bindgen/reference/deployment.html
            // https://rustwasm.github.io/wasm-pack/book/commands/build.html
            asyncWebAssembly: false
        }
    };
}


module.exports = config;
