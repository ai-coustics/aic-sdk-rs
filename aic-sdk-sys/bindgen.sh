bindgen \
  --constified-enum-module AicErrorCode \
  --constified-enum-module AicEnhancementParameter \
  --constified-enum-module AicModelType \
  --constified-enum-module AicVadParameter \
  -o src/bindings.rs \
  include/aic.h
