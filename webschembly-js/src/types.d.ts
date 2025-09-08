declare namespace WebAssembly {
  class Exception {
    is(tag: ExceptionTag): boolean;
  }

  // numberではないが ModuleImports型 にしないと imports に渡せないためworkaround
  type ExceptionTag = number & { __tagBrand: any };
}
