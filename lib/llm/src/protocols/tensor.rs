// SPDX-FileCopyrightText: Copyright (c) 2025-2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::protocols::Annotated;
use anyhow::Result;
use dynamo_runtime::protocols::annotated::AnnotationsProvider;
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use validator::Validate;

// [gluo TODO] whether it makes sense to have aggregator for tensor..
// we could if considering aggregation to be stacking the tensors by adding
// one more dimension. i.e. stream of [2, 2] tensors to be aggregated to
// [-1, 2, 2]. Will decide it later and currently do not allow aggregation.
// mod aggregator;

// pub use aggregator::DeltaAggregator;

// [gluo TODO] nvext is LLM specific, we really only use the annotation field
pub use super::openai::nvext::{NvExt, NvExtProvider};

#[derive(Debug, Serialize, Clone, Eq, PartialEq, Deserialize)]
pub enum DataType {
    Bool,
    Uint8,
    Uint16,
    Uint32,
    Uint64,
    Int8,
    Int16,
    Int32,
    Int64,
    Float32,
    Float64,
    Bytes,
}

impl DataType {
    pub fn size(&self) -> usize {
        match self {
            DataType::Bool => size_of::<bool>(),
            DataType::Uint8 => size_of::<u8>(),
            DataType::Uint16 => size_of::<u16>(),
            DataType::Uint32 => size_of::<u32>(),
            DataType::Uint64 => size_of::<u64>(),
            DataType::Int8 => size_of::<i8>(),
            DataType::Int16 => size_of::<i16>(),
            DataType::Int32 => size_of::<i32>(),
            DataType::Int64 => size_of::<i64>(),
            DataType::Float32 => size_of::<f32>(),
            DataType::Float64 => size_of::<f64>(),
            DataType::Bytes => 0, // variable length, return 0 as indicator
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq, Deserialize)]
// Self-describing encoding removes ambiguity between signed/unsigned and width variants.
#[serde(tag = "data_type", content = "values")]
pub enum FlattenTensor {
    Bool(Vec<bool>),
    // [gluo NOTE] f16, and bf16 is not stably supported
    Uint8(Vec<u8>),
    Uint16(Vec<u16>),
    Uint32(Vec<u32>),
    Uint64(Vec<u64>),
    Int8(Vec<i8>),
    Int16(Vec<i16>),
    Int32(Vec<i32>),
    Int64(Vec<i64>),
    Float32(Vec<f32>),
    Float64(Vec<f64>),
    // Typically use to store string data, but really it can store
    // arbitrary data such as serialized handles for custom worker behavior.
    Bytes(Vec<Vec<u8>>),
}

#[allow(clippy::len_without_is_empty)]
impl FlattenTensor {
    pub fn len(&self) -> usize {
        match self {
            Self::Bool(v) => v.len(),
            Self::Uint8(v) => v.len(),
            Self::Uint16(v) => v.len(),
            Self::Uint32(v) => v.len(),
            Self::Uint64(v) => v.len(),
            Self::Int8(v) => v.len(),
            Self::Int16(v) => v.len(),
            Self::Int32(v) => v.len(),
            Self::Int64(v) => v.len(),
            Self::Float32(v) => v.len(),
            Self::Float64(v) => v.len(),
            Self::Bytes(v) => v.len(),
        }
    }

    pub fn data_type(&self) -> DataType {
        match self {
            Self::Bool(_) => DataType::Bool,
            Self::Uint8(_) => DataType::Uint8,
            Self::Uint16(_) => DataType::Uint16,
            Self::Uint32(_) => DataType::Uint32,
            Self::Uint64(_) => DataType::Uint64,
            Self::Int8(_) => DataType::Int8,
            Self::Int16(_) => DataType::Int16,
            Self::Int32(_) => DataType::Int32,
            Self::Int64(_) => DataType::Int64,
            Self::Float32(_) => DataType::Float32,
            Self::Float64(_) => DataType::Float64,
            Self::Bytes(_) => DataType::Bytes,
        }
    }
}

#[derive(Serialize, Deserialize, Validate, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TensorMetadata {
    pub name: String,
    pub data_type: DataType,
    pub shape: Vec<i64>,

    /// Optional parameters for this tensor
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub parameters: Parameters,
}

#[derive(Serialize, Deserialize, Validate, Debug, Clone, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct TensorModelConfig {
    pub name: String,
    pub inputs: Vec<TensorMetadata>,
    pub outputs: Vec<TensorMetadata>,
    // Optional Triton model config in serialized protobuf string,
    // if provided, it supersedes the basic model config defined above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub triton_model_config: Option<Vec<u8>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Tensor {
    pub metadata: TensorMetadata,
    pub data: FlattenTensor,
}

impl validator::Validate for Tensor {
    fn validate(&self) -> Result<(), validator::ValidationErrors> {
        use validator::{ValidationError, ValidationErrors};
        let mut errs = ValidationErrors::new();

        // dtype must match
        if self.metadata.data_type != self.data.data_type() {
            let mut e = ValidationError::new("dtype_mismatch");
            e.message = Some("metadata.data_type does not match data variant".into());
            errs.add("data_type", e);
        }

        let mut product: usize = 1;
        for &d in &self.metadata.shape {
            if d < 0 {
                let mut e = ValidationError::new("negative_dim");
                e.message = Some("only -1 is allowed as a wildcard dimension".into());
                errs.add("shape", e);
                break;
            }
            product = product.saturating_mul(d as usize);
        }
        // bytes payloads may be variable-length per item; enforce outer count only
        let expect_count = self.data.len();
        if product != expect_count {
            let mut e = ValidationError::new("element_count_mismatch");
            e.message = Some(
                format!(
                    "shape implies {} elements but data has {}",
                    product, expect_count
                )
                .into(),
            );
            errs.add("shape", e);
        }

        if errs.is_empty() { Ok(()) } else { Err(errs) }
    }
}

#[derive(Serialize, Deserialize, Validate, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct NvCreateTensorRequest {
    /// ID of the request
    pub id: Option<String>,

    /// ID of the model to use.
    pub model: String,

    /// Input tensors.
    #[validate(nested)]
    pub tensors: Vec<Tensor>,

    /// Optional request-level parameters
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub parameters: Parameters,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub nvext: Option<NvExt>,
}

/// A response structure for unary chat completion responses, embedding OpenAI's
/// `CreateChatCompletionResponse`.
#[derive(Serialize, Deserialize, Validate, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct NvCreateTensorResponse {
    /// ID of the corresponding request.
    pub id: Option<String>,

    /// ID of the model.
    pub model: String,

    /// Output tensors.
    #[validate(nested)]
    pub tensors: Vec<Tensor>,

    /// Optional response-level parameters
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub parameters: Parameters,
}

/// Implements `NvExtProvider` for `NvCreateTensorRequest`,
/// providing access to NVIDIA-specific extensions.
impl NvExtProvider for NvCreateTensorRequest {
    fn nvext(&self) -> Option<&NvExt> {
        self.nvext.as_ref()
    }

    fn raw_prompt(&self) -> Option<String> {
        // Not really apply here.
        None
    }
}

/// Implements `AnnotationsProvider` for `NvCreateTensorRequest`,
/// enabling retrieval and management of request annotations.
impl AnnotationsProvider for NvCreateTensorRequest {
    /// Retrieves the list of annotations from `NvExt`, if present.
    fn annotations(&self) -> Option<Vec<String>> {
        self.nvext
            .as_ref()
            .and_then(|nvext| nvext.annotations.clone())
    }

    /// Checks whether a specific annotation exists in the request.
    ///
    /// # Arguments
    /// * `annotation` - A string slice representing the annotation to check.
    ///
    /// # Returns
    /// `true` if the annotation exists, `false` otherwise.
    fn has_annotation(&self, annotation: &str) -> bool {
        self.nvext
            .as_ref()
            .and_then(|nvext| nvext.annotations.as_ref())
            .map(|annotations| annotations.contains(&annotation.to_string()))
            .unwrap_or(false)
    }
}

pub struct DeltaAggregator {
    response: Option<NvCreateTensorResponse>,
    error: Option<String>,
}

impl NvCreateTensorResponse {
    pub async fn from_annotated_stream(
        stream: impl Stream<Item = Annotated<NvCreateTensorResponse>>,
    ) -> Result<NvCreateTensorResponse> {
        let aggregator = stream
            .fold(
                DeltaAggregator {
                    response: None,
                    error: None,
                },
                |mut aggregator, delta| async move {
                    let delta = match delta.ok() {
                        Ok(delta) => delta,
                        Err(error) => {
                            if aggregator.error.is_none() {
                                aggregator.error = Some(error);
                            }
                            return aggregator;
                        }
                    };
                    match delta.data {
                        Some(resp) => {
                            if aggregator.response.is_none() {
                                aggregator.response = Some(resp);
                            } else if aggregator.error.is_none() {
                                aggregator.error =
                                    Some("Multiple responses in non-streaming mode".to_string());
                            }
                        }
                        None => {
                            // Ignore metadata-only deltas in non-streaming mode.
                        }
                    }
                    aggregator
                },
            )
            .await;
        if let Some(error) = aggregator.error {
            Err(anyhow::anyhow!(error))
        } else if let Some(response) = aggregator.response {
            Ok(response)
        } else {
            Err(anyhow::anyhow!("No response received"))
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ParameterValue {
    Bool(bool),
    Int64(i64),
    String(String),
    Double(f64),
    Uint64(u64),
}

pub type Parameters = HashMap<String, ParameterValue>;

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    // Tensor::validate regression tests

    #[test]
    fn test_tensor_empty_shape_with_empty_data() {
        // shape [] implies product=1, but empty data has 0 elements → mismatch
        let tensor = Tensor {
            metadata: TensorMetadata {
                name: "test".to_string(),
                data_type: DataType::Float32,
                shape: vec![],
                parameters: HashMap::new(),
            },
            data: FlattenTensor::Float32(vec![]),
        };
        let result = tensor.validate();
        // product of empty shape = 1, data len = 0 → should fail
        assert!(result.is_err());
    }

    #[test]
    fn test_tensor_empty_shape_with_one_element() {
        // shape [] with product=1 and data with 1 element → should pass
        let tensor = Tensor {
            metadata: TensorMetadata {
                name: "test".to_string(),
                data_type: DataType::Float32,
                shape: vec![],
                parameters: HashMap::new(),
            },
            data: FlattenTensor::Float32(vec![1.0]),
        };
        let result = tensor.validate();
        // product of empty shape = 1, data len = 1 → should pass
        assert!(result.is_ok());
    }

    #[test]
    fn test_tensor_shape_with_zero_dimension() {
        // shape [2, 0, 3] implies 0 elements
        let tensor = Tensor {
            metadata: TensorMetadata {
                name: "test".to_string(),
                data_type: DataType::Int32,
                shape: vec![2, 0, 3],
                parameters: HashMap::new(),
            },
            data: FlattenTensor::Int32(vec![]),
        };
        let result = tensor.validate();
        // product = 0, data len = 0 → should pass
        assert!(result.is_ok());
    }

    #[test]
    fn test_tensor_shape_with_zero_but_nonempty_data() {
        // shape [0] implies 0 elements but data has elements → mismatch
        let tensor = Tensor {
            metadata: TensorMetadata {
                name: "test".to_string(),
                data_type: DataType::Int32,
                shape: vec![0],
                parameters: HashMap::new(),
            },
            data: FlattenTensor::Int32(vec![1, 2, 3]),
        };
        let result = tensor.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_tensor_dtype_mismatch() {
        // metadata says Float32 but data is Int32
        let tensor = Tensor {
            metadata: TensorMetadata {
                name: "test".to_string(),
                data_type: DataType::Float32,
                shape: vec![2],
                parameters: HashMap::new(),
            },
            data: FlattenTensor::Int32(vec![1, 2]),
        };
        let result = tensor.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_tensor_negative_dimension() {
        // Negative dimensions (other than -1 wildcard) should be rejected
        let tensor = Tensor {
            metadata: TensorMetadata {
                name: "test".to_string(),
                data_type: DataType::Float32,
                shape: vec![-2, 3],
                parameters: HashMap::new(),
            },
            data: FlattenTensor::Float32(vec![1.0; 6]),
        };
        let result = tensor.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_tensor_valid_2d() {
        let tensor = Tensor {
            metadata: TensorMetadata {
                name: "matrix".to_string(),
                data_type: DataType::Float64,
                shape: vec![2, 3],
                parameters: HashMap::new(),
            },
            data: FlattenTensor::Float64(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]),
        };
        assert!(tensor.validate().is_ok());
    }

    #[test]
    fn test_tensor_element_count_mismatch() {
        // shape [2,3] = 6 elements but data has 5
        let tensor = Tensor {
            metadata: TensorMetadata {
                name: "test".to_string(),
                data_type: DataType::Float32,
                shape: vec![2, 3],
                parameters: HashMap::new(),
            },
            data: FlattenTensor::Float32(vec![1.0; 5]),
        };
        assert!(tensor.validate().is_err());
    }

    #[test]
    fn test_tensor_bytes_type() {
        // Bytes type with shape and data
        let tensor = Tensor {
            metadata: TensorMetadata {
                name: "strings".to_string(),
                data_type: DataType::Bytes,
                shape: vec![3],
                parameters: HashMap::new(),
            },
            data: FlattenTensor::Bytes(vec![
                b"hello".to_vec(),
                b"world".to_vec(),
                b"!".to_vec(),
            ]),
        };
        assert!(tensor.validate().is_ok());
    }

    // FlattenTensor deserialization tests

    #[test]
    fn test_flatten_tensor_roundtrip_json() {
        let tensor = FlattenTensor::Int64(vec![1, 2, 3]);
        let json = serde_json::to_string(&tensor).unwrap();
        let decoded: FlattenTensor = serde_json::from_str(&json).unwrap();
        assert_eq!(tensor, decoded);
    }

    #[test]
    fn test_flatten_tensor_bool_roundtrip() {
        let tensor = FlattenTensor::Bool(vec![true, false, true]);
        let json = serde_json::to_string(&tensor).unwrap();
        let decoded: FlattenTensor = serde_json::from_str(&json).unwrap();
        assert_eq!(tensor, decoded);
    }

    #[test]
    fn test_flatten_tensor_empty_values() {
        let tensor = FlattenTensor::Float32(vec![]);
        let json = serde_json::to_string(&tensor).unwrap();
        let decoded: FlattenTensor = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.len(), 0);
    }

    #[test]
    fn test_flatten_tensor_invalid_json() {
        // Missing required fields
        let result: Result<FlattenTensor, _> = serde_json::from_str("{}");
        assert!(result.is_err());

        // Invalid data_type tag
        let result: Result<FlattenTensor, _> =
            serde_json::from_str(r#"{"data_type": "Unknown", "values": []}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_flatten_tensor_wrong_value_types() {
        // data_type says Int32 but values are strings
        let result: Result<FlattenTensor, _> =
            serde_json::from_str(r#"{"data_type": "Int32", "values": ["a", "b"]}"#);
        assert!(result.is_err());
    }

    // DataType::size tests

    #[test]
    fn test_data_type_sizes() {
        assert_eq!(DataType::Bool.size(), 1);
        assert_eq!(DataType::Uint8.size(), 1);
        assert_eq!(DataType::Uint16.size(), 2);
        assert_eq!(DataType::Uint32.size(), 4);
        assert_eq!(DataType::Uint64.size(), 8);
        assert_eq!(DataType::Int8.size(), 1);
        assert_eq!(DataType::Int16.size(), 2);
        assert_eq!(DataType::Int32.size(), 4);
        assert_eq!(DataType::Int64.size(), 8);
        assert_eq!(DataType::Float32.size(), 4);
        assert_eq!(DataType::Float64.size(), 8);
        assert_eq!(DataType::Bytes.size(), 0); // variable length
    }

    // TensorMetadata deserialization with deny_unknown_fields

    #[test]
    fn test_tensor_metadata_unknown_fields_rejected() {
        let result: Result<TensorMetadata, _> = serde_json::from_str(
            r#"{"name": "t", "data_type": "Float32", "shape": [1], "extra": true}"#,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_tensor_metadata_valid() {
        let result: Result<TensorMetadata, _> = serde_json::from_str(
            r#"{"name": "t", "data_type": "Float32", "shape": [2, 3]}"#,
        );
        assert!(result.is_ok());
        let meta = result.unwrap();
        assert_eq!(meta.name, "t");
        assert_eq!(meta.shape, vec![2, 3]);
    }

    // ParameterValue deserialization edge cases

    #[test]
    fn test_parameter_value_variants() {
        let val: ParameterValue = serde_json::from_str(r#"{"bool": true}"#).unwrap();
        assert_eq!(val, ParameterValue::Bool(true));

        let val: ParameterValue = serde_json::from_str(r#"{"int64": -42}"#).unwrap();
        assert_eq!(val, ParameterValue::Int64(-42));

        let val: ParameterValue =
            serde_json::from_str(r#"{"string": "hello"}"#).unwrap();
        assert_eq!(val, ParameterValue::String("hello".to_string()));

        let val: ParameterValue = serde_json::from_str(r#"{"double": 3.14}"#).unwrap();
        assert_eq!(val, ParameterValue::Double(3.14));

        let val: ParameterValue = serde_json::from_str(r#"{"uint64": 999}"#).unwrap();
        assert_eq!(val, ParameterValue::Uint64(999));
    }
}
