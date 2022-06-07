
let seedapp_promise: Promise<any> = import('./pkg/seedapp_rust');
let seedapp: any = null;
seedapp_promise.then((sa) => {
    seedapp = sa.default();
})

if ((module as any).hot) {
    (module as any).hot.accept();
    (module as any).hot.dispose(async () => {
        (await seedapp).dispose();
    });
}
