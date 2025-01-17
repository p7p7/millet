(*!
 * The Array structure defines polymorphic arrays, mutable sequences with constant-time access and
 * update.
 *
 * Arrays have a special equality property: two arrays are equal if they are the same array, i.e.,
 * created by the same call to a primitive array constructor such as array, fromList, etc.;
 * otherwise they are not equal. This also holds for arrays of zero length. Thus, the type ty array
 * admits equality even if ty does not.
 *)
signature ARRAY = sig
  eqtype 'a array
  type 'a vector = 'a Vector.vector
  (*!
   * The maximum length of arrays supported by this implementation. Attempts to create larger arrays
   * will result in the Size exception being raised.
   *)
  val maxLen : int
  (*!
   * array (n, init) creates a new array of length n; each element is initialized to the value init.
   * If n < 0 or maxLen < n, then the Size exception is raised.
   *)
  val array : int * 'a -> 'a array
  (*!
   * fromList l creates a new array from l. The length of the array is length l and the i(th)
   * element of the array is the i(th) element of the the list. If the length of the list is greater
   * than maxLen, then the Size exception is raised.
   *)
  val fromList : 'a list -> 'a array
  (*!
   * tabulate (n, f) creates an array of n elements, where the elements are defined in order of
   * increasing index by applying f to the element's index. This is equivalent to the expression:
   * fromList (List.tabulate (n, f)) If n < 0 or maxLen < n, then the Size exception is raised.
   *)
  val tabulate : int * (int -> 'a) -> 'a array
  (*!
   * length arr returns |arr|, the length of the array arr.
   *)
  val length : 'a array -> int
  (*!
   * sub (arr, i) returns the i(th) element of the array arr. If i < 0 or |arr| <= i, then the
   * Subscript exception is raised.
   *)
  val sub : 'a array * int -> 'a
  (*!
   * update (arr, i, x) sets the i(th) element of the array arr to x. If i < 0 or |arr| <= i, then
   * the Subscript exception is raised.
   *)
  val update : 'a array * int * 'a -> unit
  (*!
   * vector arr generates a vector from arr. Specifically, the result is equivalent to
   * Vector.tabulate (length arr, fn i => sub (arr, i))
   *)
  val vector : 'a array -> 'a vector
  (*!
   * These functions copy the entire array or vector src into the array dst, with the i(th) element
   * in src, for 0 <= i < |src|, being copied to position di + i in the destination array. If di < 0
   * or if |dst| < di+|src|, then the Subscript exception is raised. Implementation note: In copy,
   * if dst and src are equal, we must have di = 0 to avoid an exception, and copy is then the
   * identity.
   *)
  val copy : {src : 'a array, dst : 'a array, di : int} -> unit
  (*!
   * See copy.
   *)
  val copyVec : {src : 'a vector, dst : 'a array, di : int} -> unit
  (*!
   * These apply the function f to the elements of the array arr in order of increasing indices. The
   * more general form appi supplies f with the array index of the corresponding element.
   *)
  val appi : (int * 'a -> unit) -> 'a array -> unit
  (*!
   * See appi.
   *)
  val app : ('a -> unit) -> 'a array -> unit
  (*!
   * These apply the function f to the elements of the array arr in order of increasing indices, and
   * replace each element with the result. The more general modifyi supplies f with the array index
   * of the corresponding element. The expression modify f arr is equivalent to modifyi (f o #2)
   * arr.
   *)
  val modifyi : (int * 'a -> 'a) -> 'a array -> unit
  (*!
   * See modifyi.
   *)
  val modify : ('a -> 'a) -> 'a array -> unit
  (*!
   * These fold the function f over all the elements of the array arr, using the value init as the
   * initial value. The functions foldli and foldl apply the function f from left to right
   * (increasing indices), while the functions foldri and foldr work from right to left (decreasing
   * indices). The more general functions foldli and foldri supply f with the array index of the
   * corresponding element. Refer to the MONO_ARRAY manual pages for reference implementations of
   * the indexed versions. The expression foldl f init arr is equivalent to: foldli (fn (_, a, x) =>
   * f(a, x)) init arr The analogous equivalences hold for foldri and foldr.
   *)
  val foldli : (int * 'a * 'b -> 'b) -> 'b -> 'a array -> 'b
  (*!
   * See foldli.
   *)
  val foldri : (int * 'a * 'b -> 'b) -> 'b -> 'a array -> 'b
  (*!
   * See foldli.
   *)
  val foldl : ('a * 'b -> 'b) -> 'b -> 'a array -> 'b
  (*!
   * See foldli.
   *)
  val foldr : ('a * 'b -> 'b) -> 'b -> 'a array -> 'b
  (*!
   * These functions apply f to each element of the array arr, from left to right (i.e., increasing
   * indices), until a true value is returned. If this occurs, the functions return the element;
   * otherwise, they return NONE. The more general version findi also supplies f with the array
   * index of the element and, upon finding an entry satisfying the predicate, returns that index
   * with the element.
   *)
  val findi : (int * 'a -> bool) -> 'a array -> (int * 'a) option
  (*!
   * See findi.
   *)
  val find : ('a -> bool) -> 'a array -> 'a option
  (*!
   * exists f arr applies f to each element x of the array arr, from left to right (i.e., increasing
   * indices), until f x evaluates to true; it returns true if such an x exists and false otherwise.
   *)
  val exists : ('a -> bool) -> 'a array -> bool
  (*!
   * all f arr applies f to each element x of the array arr, from left to right (i.e., increasing
   * indices), until f x evaluates to false; it returns false if such an x exists and true
   * otherwise. It is equivalent to not(exists (not o f) arr)).
   *)
  val all : ('a -> bool) -> 'a array -> bool
  (*!
   * collate f (a1, a2) performs lexicographic comparison of the two arrays using the given ordering
   * f on elements.
   *)
  val collate : ('a * 'a -> order) -> 'a array * 'a array -> order
end

structure Array :> ARRAY = struct end
datatype array = datatype Array.array
val vector = Array.vector
