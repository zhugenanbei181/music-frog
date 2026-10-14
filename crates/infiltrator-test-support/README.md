# infiltrator-test-support

> 层级：L3

This crate owns engine-neutral fake ports, in-memory stores, and deterministic
documents shared by Core tests and isolated UI captures. It depends only on
contract, domain, and ports. It does not create product runtimes or select an
engine adapter; product compositions inject concrete codecs separately.

Application uses the crate only as a dev-dependency. `infiltrator-composition`
enables it behind `capture-fixtures`; Iced and Bevy capture tools consume the
same fixture definitions without reaching into Application's test modules.
