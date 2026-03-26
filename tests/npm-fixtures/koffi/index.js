const koffi = {
  opaque() {
    return { kind: "opaque" };
  },
  pointer(typeOrName, maybeType) {
    return {
      kind: "pointer",
      name: maybeType == null ? null : typeOrName,
      to: maybeType ?? typeOrName,
    };
  },
  struct(name, fields) {
    return {
      kind: "struct",
      name,
      fields,
    };
  },
  proto(name, returnType, argumentTypes) {
    return {
      kind: "proto",
      name,
      returnType,
      argumentTypes,
    };
  },
  load() {
    return {
      func(name) {
        return (..._args) => {
          throw new Error(`mock koffi function ${name} is not implemented`);
        };
      },
      unload() {},
    };
  },
};

export default koffi;
