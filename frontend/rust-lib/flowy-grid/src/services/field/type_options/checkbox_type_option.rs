use crate::impl_type_option;
use crate::services::field::{BoxTypeOptionBuilder, TypeOptionBuilder};
use crate::services::row::{CellContentChangeset, CellDataOperation, DecodedCellData};
use bytes::Bytes;
use flowy_derive::ProtoBuf;
use flowy_error::{FlowyError, FlowyResult};
use flowy_grid_data_model::entities::FieldType;
use flowy_grid_data_model::revision::{CellRevision, FieldRevision, TypeOptionDataDeserializer, TypeOptionDataEntry};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct CheckboxTypeOptionBuilder(CheckboxTypeOption);
impl_into_box_type_option_builder!(CheckboxTypeOptionBuilder);
impl_builder_from_json_str_and_from_bytes!(CheckboxTypeOptionBuilder, CheckboxTypeOption);

impl CheckboxTypeOptionBuilder {
    pub fn set_selected(mut self, is_selected: bool) -> Self {
        self.0.is_selected = is_selected;
        self
    }
}

impl TypeOptionBuilder for CheckboxTypeOptionBuilder {
    fn field_type(&self) -> FieldType {
        self.0.field_type()
    }

    fn entry(&self) -> &dyn TypeOptionDataEntry {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ProtoBuf)]
pub struct CheckboxTypeOption {
    #[pb(index = 1)]
    pub is_selected: bool,
}
impl_type_option!(CheckboxTypeOption, FieldType::Checkbox);

const YES: &str = "Yes";
const NO: &str = "No";

impl CellDataOperation<String> for CheckboxTypeOption {
    fn decode_cell_data<T>(
        &self,
        encoded_data: T,
        decoded_field_type: &FieldType,
        _field_rev: &FieldRevision,
    ) -> FlowyResult<DecodedCellData>
    where
        T: Into<String>,
    {
        if !decoded_field_type.is_checkbox() {
            return Ok(DecodedCellData::default());
        }

        let encoded_data = encoded_data.into();
        if encoded_data == YES || encoded_data == NO {
            return Ok(DecodedCellData::new(encoded_data));
        }

        Ok(DecodedCellData::default())
    }

    fn apply_changeset<C>(&self, changeset: C, _cell_rev: Option<CellRevision>) -> Result<String, FlowyError>
    where
        C: Into<CellContentChangeset>,
    {
        let changeset = changeset.into();
        let s = match string_to_bool(&changeset) {
            true => YES,
            false => NO,
        };
        Ok(s.to_string())
    }
}

fn string_to_bool(bool_str: &str) -> bool {
    let lower_case_str: &str = &bool_str.to_lowercase();
    match lower_case_str {
        "1" => true,
        "true" => true,
        "yes" => true,
        "0" => false,
        "false" => false,
        "no" => false,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::services::field::type_options::checkbox_type_option::{NO, YES};

    use crate::services::field::FieldBuilder;
    use crate::services::row::{apply_cell_data_changeset, decode_cell_data};

    use flowy_grid_data_model::entities::FieldType;

    #[test]
    fn checkout_box_description_test() {
        let field_rev = FieldBuilder::from_field_type(&FieldType::Checkbox).build();
        let data = apply_cell_data_changeset("true", None, &field_rev).unwrap();
        assert_eq!(decode_cell_data(data, &field_rev).to_string(), YES);

        let data = apply_cell_data_changeset("1", None, &field_rev).unwrap();
        assert_eq!(decode_cell_data(data, &field_rev,).to_string(), YES);

        let data = apply_cell_data_changeset("yes", None, &field_rev).unwrap();
        assert_eq!(decode_cell_data(data, &field_rev,).to_string(), YES);

        let data = apply_cell_data_changeset("false", None, &field_rev).unwrap();
        assert_eq!(decode_cell_data(data, &field_rev,).to_string(), NO);

        let data = apply_cell_data_changeset("no", None, &field_rev).unwrap();
        assert_eq!(decode_cell_data(data, &field_rev,).to_string(), NO);

        let data = apply_cell_data_changeset("12", None, &field_rev).unwrap();
        assert_eq!(decode_cell_data(data, &field_rev,).to_string(), NO);
    }
}
