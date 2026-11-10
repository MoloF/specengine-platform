Inter, bundled in the UI's build (docs/features/design-foundation.md "Typography", W1)

The owner's decision of 2026-11-10: Inter, bundled under the OFL, with Cyrillic, and no font host.
The files are committed as they were published; no package is installed and no font is fetched at
run time. src/styles/fonts.css declares them, one @font-face per subset, each with the package's own
unicode-range.

Font:    Inter version 4.001 (its name table: "Version 4.001;git-66647c0bb", the copyright of
         The Inter Project Authors, the licence URL https://openfontlicense.org).
Source:  the npm package @fontsource-variable/inter, version 5.3.0 (published 2026-07-19), its
         files/ directory and its LICENSE; the package is the OFL build of Inter (the Google Fonts
         build of rsms/inter, Fontsource "v20"), split into subsets.
Tarball: https://registry.npmjs.org/@fontsource-variable/inter/-/inter-5.3.0.tgz
         sha512-OupL48va4JNofb97w6NYeF9S7W/kHNKM0Er8Dem5nqi4jeOLrVJDoE8tZEpnMJmtkvNbB1EIPPwHcdkF6b1oUA==
         (the registry's integrity, checked against the download)
Axis:    the variable weight axis, 100-900, upright; italic is synthesised.
Licence: SIL Open Font License 1.1, OFL.txt here (the package's LICENSE, unchanged). A byte-equal
         copy, ui/public/assets/OFL.txt, ships in the build beside the font files
         (dist/assets/OFL.txt); src/styles/fonts.test.ts holds the two equal.

SHA-256 of each file:
3100e775e8616cd2611beecfa23a4263d7037586789b43f035236a2e6fbd4c62  inter-latin-wght-normal.woff2
34b9c504cab7a73e37b746343a449132e56cf7b5481af2cb81dc74dcff25c956  inter-latin-ext-wght-normal.woff2
71d5ee93cc1e9f1d520a3a8b66456de18c7879d8df09d57fcd2eaff75fef0075  inter-cyrillic-wght-normal.woff2
ca157063339ac4ad418f214f3abfed119b0798ab4d377386ce5c9e5a7a435ebd  inter-cyrillic-ext-wght-normal.woff2
3b0a5fca3d17942cde889069889dedbbbd075e9b599968c82a95f4d944e9b345  OFL.txt

To check: shasum -a 256 ui/fonts/inter/* from the repository root.
