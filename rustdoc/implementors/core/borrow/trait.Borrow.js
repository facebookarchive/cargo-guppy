(function() {var implementors = {};
implementors["ascii"] = [{"text":"impl Borrow&lt;AsciiStr&gt; for AsciiString","synthetic":false,"types":[]}];
implementors["bstr"] = [{"text":"impl Borrow&lt;BStr&gt; for BString","synthetic":false,"types":[]}];
implementors["crossbeam_epoch"] = [{"text":"impl&lt;T:&nbsp;?Sized + Pointable&gt; Borrow&lt;T&gt; for Owned&lt;T&gt;","synthetic":false,"types":[]}];
implementors["smallvec"] = [{"text":"impl&lt;A:&nbsp;Array&gt; Borrow&lt;[&lt;A as Array&gt;::Item]&gt; for SmallVec&lt;A&gt;","synthetic":false,"types":[]}];
implementors["supercow"] = [{"text":"impl&lt;'a, OWNED, BORROWED:&nbsp;?Sized, SHARED, STORAGE, PTR&gt; Borrow&lt;BORROWED&gt; for Supercow&lt;'a, OWNED, BORROWED, SHARED, STORAGE, PTR&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;BORROWED: 'a,<br>&nbsp;&nbsp;&nbsp;&nbsp;*const BORROWED: PointerFirstRef,<br>&nbsp;&nbsp;&nbsp;&nbsp;STORAGE: OwnedStorage&lt;OWNED, SHARED&gt;,<br>&nbsp;&nbsp;&nbsp;&nbsp;PTR: PtrWrite&lt;BORROWED&gt;,<br>&nbsp;&nbsp;&nbsp;&nbsp;PTR: PtrRead&lt;BORROWED&gt;,&nbsp;</span>","synthetic":false,"types":[]}];
implementors["toml"] = [{"text":"impl Borrow&lt;str&gt; for Spanned&lt;String&gt;","synthetic":false,"types":[]}];
if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()