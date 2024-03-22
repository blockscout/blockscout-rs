// These types are taken from typescript/lib/lib.es5.d.ts
// Needed to make the compiler happy

type ThisParameterType<T> = T extends (this: infer U, ...args: never) => any
  ? U
  : unknown;
type OmitThisParameter<T> = unknown extends ThisParameterType<T>
  ? T
  : T extends (...args: infer A) => infer R
  ? (...args: A) => R
  : T;

declare interface CallableFunction extends Function {
  /**
   * Calls the function with the specified object as the this value and the elements of specified array as the arguments.
   * @param thisArg The object to be used as the this object.
   * @param args An array of argument values to be passed to the function.
   */
  apply<T, R>(this: (this: T) => R, thisArg: T): R;
  apply<T, A extends any[], R>(
    this: (this: T, ...args: A) => R,
    thisArg: T,
    args: A
  ): R;

  /**
   * Calls the function with the specified object as the this value and the specified rest arguments as the arguments.
   * @param thisArg The object to be used as the this object.
   * @param args Argument values to be passed to the function.
   */
  call<T, A extends any[], R>(
    this: (this: T, ...args: A) => R,
    thisArg: T,
    ...args: A
  ): R;

  /**
   * For a given function, creates a bound function that has the same body as the original function.
   * The this object of the bound function is associated with the specified object, and has the specified initial parameters.
   * @param thisArg The object to be used as the this object.
   * @param args Arguments to bind to the parameters of the function.
   */
  bind<T>(this: T, thisArg: ThisParameterType<T>): OmitThisParameter<T>;
  bind<T, A0, A extends any[], R>(
    this: (this: T, arg0: A0, ...args: A) => R,
    thisArg: T,
    arg0: A0
  ): (...args: A) => R;
  bind<T, A0, A1, A extends any[], R>(
    this: (this: T, arg0: A0, arg1: A1, ...args: A) => R,
    thisArg: T,
    arg0: A0,
    arg1: A1
  ): (...args: A) => R;
  bind<T, A0, A1, A2, A extends any[], R>(
    this: (this: T, arg0: A0, arg1: A1, arg2: A2, ...args: A) => R,
    thisArg: T,
    arg0: A0,
    arg1: A1,
    arg2: A2
  ): (...args: A) => R;
  bind<T, A0, A1, A2, A3, A extends any[], R>(
    this: (this: T, arg0: A0, arg1: A1, arg2: A2, arg3: A3, ...args: A) => R,
    thisArg: T,
    arg0: A0,
    arg1: A1,
    arg2: A2,
    arg3: A3
  ): (...args: A) => R;
  bind<T, AX, R>(
    this: (this: T, ...args: AX[]) => R,
    thisArg: T,
    ...args: AX[]
  ): (...args: AX[]) => R;
}

declare interface NewableFunction extends Function {
  /**
   * Calls the function with the specified object as the this value and the elements of specified array as the arguments.
   * @param thisArg The object to be used as the this object.
   * @param args An array of argument values to be passed to the function.
   */
  apply<T>(this: new () => T, thisArg: T): void;
  apply<T, A extends any[]>(
    this: new (...args: A) => T,
    thisArg: T,
    args: A
  ): void;

  /**
   * Calls the function with the specified object as the this value and the specified rest arguments as the arguments.
   * @param thisArg The object to be used as the this object.
   * @param args Argument values to be passed to the function.
   */
  call<T, A extends any[]>(
    this: new (...args: A) => T,
    thisArg: T,
    ...args: A
  ): void;

  /**
   * For a given function, creates a bound function that has the same body as the original function.
   * The this object of the bound function is associated with the specified object, and has the specified initial parameters.
   * @param thisArg The object to be used as the this object.
   * @param args Arguments to bind to the parameters of the function.
   */
  bind<T>(this: T, thisArg: any): T;
  bind<A0, A extends any[], R>(
    this: new (arg0: A0, ...args: A) => R,
    thisArg: any,
    arg0: A0
  ): new (...args: A) => R;
  bind<A0, A1, A extends any[], R>(
    this: new (arg0: A0, arg1: A1, ...args: A) => R,
    thisArg: any,
    arg0: A0,
    arg1: A1
  ): new (...args: A) => R;
  bind<A0, A1, A2, A extends any[], R>(
    this: new (arg0: A0, arg1: A1, arg2: A2, ...args: A) => R,
    thisArg: any,
    arg0: A0,
    arg1: A1,
    arg2: A2
  ): new (...args: A) => R;
  bind<A0, A1, A2, A3, A extends any[], R>(
    this: new (arg0: A0, arg1: A1, arg2: A2, arg3: A3, ...args: A) => R,
    thisArg: any,
    arg0: A0,
    arg1: A1,
    arg2: A2,
    arg3: A3
  ): new (...args: A) => R;
  bind<AX, R>(
    this: new (...args: AX[]) => R,
    thisArg: any,
    ...args: AX[]
  ): new (...args: AX[]) => R;
}
