#![cfg_attr(rustfmt, rustfmt::skip)]
use bytes::Bytes;
use flowy_grid::services::field::*;
use flowy_grid::services::grid_editor::{GridPadBuilder, GridRevisionEditor};
use flowy_grid::services::row::CreateRowRevisionPayload;
use flowy_grid::services::setting::GridSettingChangesetBuilder;
use flowy_grid_data_model::entities::*;
use flowy_grid_data_model::revision::*;
use flowy_revision::REVISION_WRITE_INTERVAL_IN_MILLIS;
use flowy_sync::client_grid::GridBuilder;
use flowy_test::helper::ViewTest;
use flowy_test::FlowySDKTest;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use strum::EnumCount;
use tokio::time::sleep;

pub enum EditorScript {
    CreateField {
        params: InsertFieldParams,
    },
    UpdateField {
        changeset: FieldChangesetParams,
    },
    DeleteField {
        field_rev: FieldRevision,
    },
    AssertFieldCount(usize),
    AssertFieldEqual {
        field_index: usize,
        field_rev: FieldRevision,
    },
    CreateBlock {
        block: GridBlockMetaRevision,
    },
    UpdateBlock {
        changeset: GridBlockMetaRevisionChangeset,
    },
    AssertBlockCount(usize),
    AssertBlock {
        block_index: usize,
        row_count: i32,
        start_row_index: i32,
    },
    AssertBlockEqual {
        block_index: usize,
        block: GridBlockMetaRevision,
    },
    CreateEmptyRow,
    CreateRow {
        payload: CreateRowRevisionPayload,
    },
    UpdateRow {
        changeset: RowMetaChangeset,
    },
    AssertRow {
        expected_row: RowRevision,
    },
    DeleteRows {
        row_ids: Vec<String>,
    },
    UpdateCell {
        changeset: CellChangeset,
        is_err: bool,
    },
    AssertRowCount(usize),
    #[allow(dead_code)]
    UpdateGridSetting {
        params: GridSettingChangesetParams,
    },
    InsertGridTableFilter {
        payload: CreateGridFilterPayload,
    },
    AssertTableFilterCount {
        count: i32,
    },
    DeleteGridTableFilter {
        filter_id: String,
    },
    #[allow(dead_code)]
    AssertGridSetting {
        expected_setting: GridSetting,
    },
    AssertGridRevisionPad,
}

pub struct GridEditorTest {
    pub sdk: FlowySDKTest,
    pub grid_id: String,
    pub editor: Arc<GridRevisionEditor>,
    pub field_revs: Vec<FieldRevision>,
    pub block_meta_revs: Vec<Arc<GridBlockMetaRevision>>,
    pub row_revs: Vec<Arc<RowRevision>>,
    pub field_count: usize,

    pub row_order_by_row_id: HashMap<String, RowOrder>,
}

impl GridEditorTest {
    pub async fn new() -> Self {
        let sdk = FlowySDKTest::default();
        let _ = sdk.init_user().await;
        let build_context = make_all_field_test_grid();
        let view_data: Bytes = build_context.into();
        let test = ViewTest::new_grid_view(&sdk, view_data.to_vec()).await;
        let editor = sdk.grid_manager.open_grid(&test.view.id).await.unwrap();
        let field_revs = editor.get_field_revs::<FieldOrder>(None).await.unwrap();
        let block_meta_revs = editor.get_block_meta_revs().await.unwrap();
        let row_revs = editor.grid_block_snapshots(None).await.unwrap().pop().unwrap().row_revs;
        assert_eq!(row_revs.len(), 3);
        assert_eq!(block_meta_revs.len(), 1);

        // It seems like you should add the field in the make_test_grid() function.
        // Because we assert the initialize count of the fields is equal to FieldType::COUNT.
        assert_eq!(field_revs.len(), FieldType::COUNT);

        let grid_id = test.view.id;
        Self {
            sdk,
            grid_id,
            editor,
            field_revs,
            block_meta_revs,
            row_revs,
            field_count: FieldType::COUNT,
            row_order_by_row_id: HashMap::default(),
        }
    }

    pub async fn run_scripts(&mut self, scripts: Vec<EditorScript>) {
        for script in scripts {
            self.run_script(script).await;
        }
    }

    pub async fn run_script(&mut self, script: EditorScript) {
        let grid_manager = self.sdk.grid_manager.clone();
        let pool = self.sdk.user_session.db_pool().unwrap();
        let rev_manager = self.editor.rev_manager();
        let _cache = rev_manager.revision_cache().await;

        match script {
            EditorScript::CreateField { params } => {
                if !self.editor.contain_field(&params.field.id).await {
                    self.field_count += 1;
                }

                self.editor.insert_field(params).await.unwrap();
                self.field_revs = self.editor.get_field_revs::<FieldOrder>(None).await.unwrap();
                assert_eq!(self.field_count, self.field_revs.len());
            }
            EditorScript::UpdateField { changeset: change } => {
                self.editor.update_field(change).await.unwrap();
                self.field_revs = self.editor.get_field_revs::<FieldOrder>(None).await.unwrap();
            }
            EditorScript::DeleteField { field_rev } => {
                if self.editor.contain_field(&field_rev.id).await {
                    self.field_count -= 1;
                }

                self.editor.delete_field(&field_rev.id).await.unwrap();
                self.field_revs = self.editor.get_field_revs::<FieldOrder>(None).await.unwrap();
                assert_eq!(self.field_count, self.field_revs.len());
            }
            EditorScript::AssertFieldCount(count) => {
                assert_eq!(
                    self.editor.get_field_revs::<FieldOrder>(None).await.unwrap().len(),
                    count
                );
            }
            EditorScript::AssertFieldEqual { field_index, field_rev } => {
                let field_revs = self.editor.get_field_revs::<FieldOrder>(None).await.unwrap();
                assert_eq!(field_revs[field_index].clone(), field_rev);
            }
            EditorScript::CreateBlock { block } => {
                self.editor.create_block(block).await.unwrap();
                self.block_meta_revs = self.editor.get_block_meta_revs().await.unwrap();
            }
            EditorScript::UpdateBlock { changeset: change } => {
                self.editor.update_block(change).await.unwrap();
            }
            EditorScript::AssertBlockCount(count) => {
                assert_eq!(self.editor.get_block_meta_revs().await.unwrap().len(), count);
            }
            EditorScript::AssertBlock {
                block_index,
                row_count,
                start_row_index,
            } => {
                assert_eq!(self.block_meta_revs[block_index].row_count, row_count);
                assert_eq!(self.block_meta_revs[block_index].start_row_index, start_row_index);
            }
            EditorScript::AssertBlockEqual { block_index, block } => {
                let blocks = self.editor.get_block_meta_revs().await.unwrap();
                let compared_block = blocks[block_index].clone();
                assert_eq!(compared_block, Arc::new(block));
            }
            EditorScript::CreateEmptyRow => {
                let row_order = self.editor.create_row(None).await.unwrap();
                self.row_order_by_row_id.insert(row_order.row_id.clone(), row_order);
                self.row_revs = self.get_row_revs().await;
                self.block_meta_revs = self.editor.get_block_meta_revs().await.unwrap();
            }
            EditorScript::CreateRow { payload: context } => {
                let row_orders = self.editor.insert_rows(vec![context]).await.unwrap();
                for row_order in row_orders {
                    self.row_order_by_row_id.insert(row_order.row_id.clone(), row_order);
                }
                self.row_revs = self.get_row_revs().await;
                self.block_meta_revs = self.editor.get_block_meta_revs().await.unwrap();
            }
            EditorScript::UpdateRow { changeset: change } => self.editor.update_row(change).await.unwrap(),
            EditorScript::DeleteRows { row_ids } => {
                let row_orders = row_ids
                    .into_iter()
                    .map(|row_id| self.row_order_by_row_id.get(&row_id).unwrap().clone())
                    .collect::<Vec<RowOrder>>();

                self.editor.delete_rows(row_orders).await.unwrap();
                self.row_revs = self.get_row_revs().await;
                self.block_meta_revs = self.editor.get_block_meta_revs().await.unwrap();
            }
            EditorScript::AssertRow { expected_row } => {
                let row = &*self
                    .row_revs
                    .iter()
                    .find(|row| row.id == expected_row.id)
                    .cloned()
                    .unwrap();
                assert_eq!(&expected_row, row);
                // if let Some(visibility) = changeset.visibility {
                //     assert_eq!(row.visibility, visibility);
                // }
                //
                // if let Some(height) = changeset.height {
                //     assert_eq!(row.height, height);
                // }
            }
            EditorScript::UpdateCell { changeset, is_err } => {
                let result = self.editor.update_cell(changeset).await;
                if is_err {
                    assert!(result.is_err())
                } else {
                    let _ = result.unwrap();
                    self.row_revs = self.get_row_revs().await;
                }
            }
            EditorScript::AssertRowCount(expected_row_count) => {
                assert_eq!(expected_row_count, self.row_revs.len());
            }
            EditorScript::UpdateGridSetting { params } => {
                let _ = self.editor.update_grid_setting(params).await.unwrap();
            }
            EditorScript::InsertGridTableFilter { payload } => {
                let params: CreateGridFilterParams = payload.try_into().unwrap();
                let layout_type = GridLayoutType::Table;
                let params = GridSettingChangesetBuilder::new(&self.grid_id, &layout_type)
                    .insert_filter(params)
                    .build();
                let _ = self.editor.update_grid_setting(params).await.unwrap();
            }
            EditorScript::AssertTableFilterCount { count } => {
                let layout_type = GridLayoutType::Table;
                let filters = self.editor.get_grid_filter(&layout_type).await.unwrap();
                assert_eq!(count as usize, filters.len());
            }
            EditorScript::DeleteGridTableFilter { filter_id } => {
                let layout_type = GridLayoutType::Table;
                let params = GridSettingChangesetBuilder::new(&self.grid_id, &layout_type)
                    .delete_filter(&filter_id)
                    .build();
                let _ = self.editor.update_grid_setting(params).await.unwrap();
            }
            EditorScript::AssertGridSetting { expected_setting } => {
                let setting = self.editor.get_grid_setting().await.unwrap();
                assert_eq!(expected_setting, setting);
            }
            EditorScript::AssertGridRevisionPad => {
                sleep(Duration::from_millis(2 * REVISION_WRITE_INTERVAL_IN_MILLIS)).await;
                let mut grid_rev_manager = grid_manager.make_grid_rev_manager(&self.grid_id, pool.clone()).unwrap();
                let grid_pad = grid_rev_manager.load::<GridPadBuilder>(None).await.unwrap();
                println!("{}", grid_pad.delta_str());
            }
        }
    }

    async fn get_row_revs(&self) -> Vec<Arc<RowRevision>> {
        self.editor
            .grid_block_snapshots(None)
            .await
            .unwrap()
            .pop()
            .unwrap()
            .row_revs
    }

    pub async fn grid_filters(&self) -> Vec<GridFilter> {
        let layout_type = GridLayoutType::Table;
        self.editor.get_grid_filter(&layout_type).await.unwrap()
    }

    pub fn text_field(&self) -> &FieldRevision {
        self.field_revs
            .iter()
            .filter(|field_rev| field_rev.field_type == FieldType::RichText)
            .collect::<Vec<_>>()
            .pop()
            .unwrap()
    }
}

fn make_all_field_test_grid() -> BuildGridContext {
    let text_field = FieldBuilder::new(RichTextTypeOptionBuilder::default())
        .name("Name")
        .visibility(true)
        .build();

    // Single Select
    let single_select = SingleSelectTypeOptionBuilder::default()
        .option(SelectOption::new("Live"))
        .option(SelectOption::new("Completed"))
        .option(SelectOption::new("Planned"))
        .option(SelectOption::new("Paused"));
    let single_select_field = FieldBuilder::new(single_select).name("Status").visibility(true).build();

    // MultiSelect
    let multi_select = MultiSelectTypeOptionBuilder::default()
        .option(SelectOption::new("Google"))
        .option(SelectOption::new("Facebook"))
        .option(SelectOption::new("Twitter"));
    let multi_select_field = FieldBuilder::new(multi_select)
        .name("Platform")
        .visibility(true)
        .build();

    // Number
    let number = NumberTypeOptionBuilder::default().set_format(NumberFormat::USD);
    let number_field = FieldBuilder::new(number).name("Price").visibility(true).build();

    // Date
    let date = DateTypeOptionBuilder::default()
        .date_format(DateFormat::US)
        .time_format(TimeFormat::TwentyFourHour);
    let date_field = FieldBuilder::new(date).name("Time").visibility(true).build();

    // Checkbox
    let checkbox = CheckboxTypeOptionBuilder::default();
    let checkbox_field = FieldBuilder::new(checkbox).name("is done").visibility(true).build();

    // URL
    let url = URLTypeOptionBuilder::default();
    let url_field = FieldBuilder::new(url).name("link").visibility(true).build();

    GridBuilder::default()
        .add_field(text_field)
        .add_field(single_select_field)
        .add_field(multi_select_field)
        .add_field(number_field)
        .add_field(date_field)
        .add_field(checkbox_field)
        .add_field(url_field)
        .add_empty_row()
        .add_empty_row()
        .add_empty_row()
        .build()
}
