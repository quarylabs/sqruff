//! Builds LSP semantic tokens from sqruff's concrete syntax tree.
//!
//! sqruff's parser already disambiguates the grammar while parsing, emitting a
//! distinct [`SyntaxKind`] for each leaf segment (keyword, literal, operator,
//! function name, ...). That means highlighting reduces to a flat
//! `SyntaxKind -> Highlight` lookup ([`classify`]) over the leaf segments, with
//! no separate query language. Kinds without syntax highlighting are explicitly
//! listed so new dialect kinds require a deliberate classification.

use lsp_types::{SemanticToken, SemanticTokenType, SemanticTokensLegend};
use sqruff_lib::core::linter::core::Linter;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::parser::segments::{ErasedSegment, Tables};

/// The highlight buckets we collapse the ~1000 [`SyntaxKind`] variants into.
///
/// The order of this list defines the indices used on the wire, and it must
/// stay in sync with [`legend`] (which maps each bucket to an LSP
/// [`SemanticTokenType`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Highlight {
    Keyword,
    String,
    Number,
    Comment,
    Operator,
    Function,
    Type,
    Variable,
    Parameter,
    Property,
    Macro,
}

/// All highlight buckets, in wire order.
const HIGHLIGHTS: [Highlight; 11] = [
    Highlight::Keyword,
    Highlight::String,
    Highlight::Number,
    Highlight::Comment,
    Highlight::Operator,
    Highlight::Function,
    Highlight::Type,
    Highlight::Variable,
    Highlight::Parameter,
    Highlight::Property,
    Highlight::Macro,
];

impl Highlight {
    /// Index of this bucket in the legend / on the wire.
    fn token_type(self) -> u32 {
        HIGHLIGHTS.iter().position(|&h| h == self).unwrap() as u32
    }

    fn semantic_token_type(self) -> SemanticTokenType {
        match self {
            Highlight::Keyword => SemanticTokenType::KEYWORD,
            Highlight::String => SemanticTokenType::STRING,
            Highlight::Number => SemanticTokenType::NUMBER,
            Highlight::Comment => SemanticTokenType::COMMENT,
            Highlight::Operator => SemanticTokenType::OPERATOR,
            Highlight::Function => SemanticTokenType::FUNCTION,
            Highlight::Type => SemanticTokenType::TYPE,
            Highlight::Variable => SemanticTokenType::VARIABLE,
            Highlight::Parameter => SemanticTokenType::PARAMETER,
            Highlight::Property => SemanticTokenType::PROPERTY,
            Highlight::Macro => SemanticTokenType::MACRO,
        }
    }
}

/// The legend advertised at initialize time. The token type ordering matches
/// [`HIGHLIGHTS`]; we expose no modifiers yet.
pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: HIGHLIGHTS.iter().map(|h| h.semantic_token_type()).collect(),
        token_modifiers: Vec::new(),
    }
}

/// Map a leaf [`SyntaxKind`] to a highlight bucket, or `None` to leave it
/// un-highlighted.
pub(crate) fn classify(kind: SyntaxKind) -> Option<Highlight> {
    use SyntaxKind::*;

    let highlight = match kind {
        // Keywords and keyword-like constants.
        Keyword | BareFunction | NullLiteral | BooleanLiteral | DatePart | DatePartWeek => {
            Highlight::Keyword
        }

        // Numeric literals.
        NumericLiteral | IntegerLiteral | DollarNumericLiteral | BitStringLiteral => {
            Highlight::Number
        }

        // String / quoted literals.
        QuotedLiteral
        | RawQuotedLiteral
        | BytesQuotedLiteral
        | SignedQuotedLiteral
        | DateConstructorLiteral
        | FileLiteral
        | DollarLiteral
        | AtSignLiteral => Highlight::String,

        // Comments.
        Comment | InlineComment | BlockComment => Highlight::Comment,

        // Operators.
        BinaryOperator
        | ComparisonOperator
        | RawComparisonOperator
        | AssignmentOperator
        | CastingOperator
        | LikeOperator
        | WalrusOperator
        | JsonOperator
        | ParameterAssigner
        | FunctionAssigner
        | SignIndicator
        | Plus
        | Minus
        | Divide
        | DoubleDivide
        | Star
        | Percent
        | Caret
        | Tilde
        | Ampersand
        | Pipe
        | VerticalBar
        | Not
        | RightArrow
        | Lambda
        | Dash => Highlight::Operator,

        // Function / procedure names.
        FunctionNameIdentifier | ProcedureNameIdentifier | SystemFunctionName => {
            Highlight::Function
        }

        // Type names.
        DataTypeIdentifier | PrimitiveType => Highlight::Type,

        // Parameters.
        Parameter => Highlight::Parameter,

        // Variables / placeholders.
        Variable | TsqlVariable | Placeholder => Highlight::Macro,

        // Property-style identifiers.
        PropertyNameIdentifier | PropertiesNakedIdentifier | WidgetNameIdentifier => {
            Highlight::Property
        }

        // Generic identifiers (column / table / object references resolve to
        // these at the leaf level).
        NakedIdentifier | QuotedIdentifier | Identifier | NakedIdentifierAll => Highlight::Variable,

        // Explicitly un-highlighted (punctuation, brackets, structural nodes,
        // meta, and grammar-only nodes). New SyntaxKind variants must be added
        // here or classified above.
        Unparsable
        | File
        | ColumnReference
        | ObjectReference
        | Expression
        | WildcardIdentifier
        | Function
        | FunctionContents
        | HavingClause
        | PathSegment
        | LimitClause
        | CubeRollupClause
        | GroupingSetsClause
        | GroupingExpressionList
        | SetClause
        | FetchClause
        | FunctionDefinition
        | AlterSequenceOptionsSegment
        | RoleReference
        | TablespaceReference
        | ExtensionReference
        | TagReference
        | ColumnDefinition
        | ColumnConstraintSegment
        | CommentClause
        | TableEndClause
        | MergeMatch
        | MergeWhenNotMatchedClause
        | MergeInsertClause
        | MergeUpdateClause
        | MergeDeleteClause
        | MergeTreeOrderByClause
        | SetClauseList
        | TableReference
        | GroupbyClause
        | FrameClause
        | WithCompoundStatement
        | CommonTableExpression
        | CTEColumnList
        | ReferencedColumnList
        | TriggerReference
        | TableConstraint
        | JoinOnCondition
        | DatabaseReference
        | CollationReference
        | OverClause
        | NamedWindow
        | WindowSpecification
        | PartitionbyClause
        | JoinClause
        | DropTriggerStatement
        | SampleExpression
        | TableExpression
        | CreateTriggerStatement
        | DropModelStatement
        | DescribeStatement
        | UseStatement
        | ExplainStatement
        | CreateSequenceStatement
        | CreateSequenceOptionsSegment
        | AlterSequenceStatement
        | DropSequenceStatement
        | DropCastStatement
        | CreateFunctionStatement
        | DropFunctionStatement
        | CreateModelStatement
        | CreateViewStatement
        | DeleteStatement
        | UpdateStatement
        | CreateCastStatement
        | CreateRoleStatement
        | DropRoleStatement
        | AlterTableStatement
        | CreateSchemaStatement
        | SetSchemaStatement
        | DropSchemaStatement
        | DropTypeStatement
        | CreateDatabaseStatement
        | DropDatabaseStatement
        | FunctionParameterList
        | CreateIndexStatement
        | DropIndexStatement
        | CreateTableStatement
        | AccessStatement
        | InsertStatement
        | TransactionStatement
        | DropTableStatement
        | DropViewStatement
        | CreateUserStatement
        | DropUserStatement
        | ArrayExpression
        | LocalAlias
        | MergeStatement
        | IndexColumnDefinition
        | AggregateOrderByClause
        | FunctionName
        | CaseExpression
        | WhenClause
        | ElseClause
        | PreWhereClause
        | WhereClause
        | SetOperator
        | ValuesClause
        | EmptyStructLiteral
        | ObjectLiteral
        | ObjectLiteralElement
        | TimeZoneGrammar
        | BracketedArguments
        | DataType
        | AliasExpression
        | ArrayAccessor
        | ArrayLiteral
        | TypedArrayLiteral
        | StructType
        | StructLiteral
        | TypedStructLiteral
        | IntervalExpression
        | ArrayType
        | SizedArrayType
        | SelectStatement
        | OverlapsClause
        | SelectClause
        | Statement
        | WithNoSchemaBindingClause
        | WithDataClause
        | SetExpression
        | FromClause
        | EmptyStructLiteralBrackets
        | WildcardExpression
        | OrderbyClause
        | TruncateStatement
        | FromExpression
        | FromExpressionElement
        | SelectClauseModifier
        | NamedWindowExpression
        | SelectClauseElement
        | QualifyClause
        | MultiStatementSegment
        | AssertStatement
        | ForInStatements
        | ForInStatement
        | RepeatStatements
        | RepeatStatement
        | IfStatements
        | IfStatement
        | LoopStatements
        | LoopStatement
        | WhileStatements
        | WhileStatement
        | SelectExceptClause
        | SelectReplaceClause
        | StructTypeSchema
        | Tuple
        | NamedArgument
        | DeclareSegment
        | SetSegment
        | PartitionBySegment
        | ClusterBySegment
        | OptionsSegment
        | CreateExternalTableStatement
        | AlterViewStatement
        | CreateMaterializedViewStatement
        | AlterMaterializedViewSetOptionsStatement
        | DropMaterializedViewStatement
        | ParameterizedExpression
        | PivotForClause
        | FromPivotExpression
        | UnpivotClause
        | FromUnpivotExpression
        | NotMatchedByTargetClause
        | MergeWhenMatchedClause
        | ProcedureName
        | ExportStatement
        | ProcedureParameterList
        | ProcedureStatements
        | CallStatement
        | ReturnStatement
        | BreakStatement
        | LeaveStatement
        | ContinueStatement
        | RaiseStatement
        | PsqlVariable
        | DatetimeTypeIdentifier
        | DatetimeLiteral
        | IndexAccessMethod
        | OperatorClassReference
        | DefinitionParameter
        | DefinitionParameters
        | RelationOption
        | RelationOptions
        | AlterFunctionActionSegment
        | AlterProcedureActionSegment
        | AlterProcedureStatement
        | DropProcedureStatement
        | WktGeometryType
        | IntoClause
        | ForClause
        | AlterRoleStatement
        | ExplainOption
        | CreateTableAsStatement
        | AlterPublicationStatement
        | CreatePublicationStatement
        | PublicationObjects
        | PublicationTable
        | PublicationReference
        | DropExtensionStatement
        | CreateExtensionStatement
        | VersionIdentifier
        | AlterTableActionSegment
        | DropPublicationStatement
        | AlterMaterializedViewStatement
        | AlterMaterializedViewActionSegment
        | RefreshMaterializedViewStatement
        | WithCheckOption
        | AlterPolicyStatement
        | AlterDatabaseStatement
        | VacuumStatement
        | LikeOptionSegment
        | PartitionBoundSpec
        | IndexParameters
        | ReferentialActionSegment
        | IndexElement
        | ExclusionConstraintElement
        | AlterDefaultPrivilegesStatement
        | AlterDefaultPrivilegesObjectPrivilege
        | AlterDefaultPrivilegesSchemaObject
        | AlterDefaultPrivilegesToFromRoles
        | AlterDefaultPrivilegesGrant
        | DropOwnedStatement
        | ReassignOwnedStatement
        | IndexElementOptions
        | AlterDefaultPrivilegesRevoke
        | AlterIndexStatement
        | ReindexStatementSegment
        | AnalyzeStatement
        | AlterTrigger
        | OperationClassReference
        | ConflictAction
        | ConflictTarget
        | SetStatement
        | CreatePolicyStatement
        | CreateDomainStatement
        | AlterDomainStatement
        | DropDomainStatement
        | DropPolicyStatement
        | LoadStatement
        | ResetStatement
        | ListenStatement
        | NotifyStatement
        | UnlistenStatement
        | ClusterStatement
        | LanguageClause
        | DoStatement
        | CreateUserMappingStatement
        | ImportForeignSchemaStatement
        | CreateServerStatement
        | CreateCollationStatement
        | AlterTypeStatement
        | CreateTypeStatement
        | LockTableStatement
        | CopyStatement
        | DiscardStatement
        | AlterSchemaStatement
        | ServerReference
        | ArrayJoinClause
        | TableEngineFunction
        | OnClusterClause
        | Engine
        | EngineFunction
        | DatabaseEngine
        | ColumnTtlSegment
        | TableTtlSegment
        | DropDictionaryStatement
        | DropQuotaStatement
        | DropSettingProfileStatement
        | SystemMergesSegment
        | SystemTtlMergesSegment
        | SystemMovesSegment
        | SystemReplicaSegment
        | SystemFilesystemSegment
        | SystemReplicatedSegment
        | SystemReplicationSegment
        | SystemFetchesSegment
        | SystemDistributedSegment
        | SystemModelSegment
        | SystemFileSegment
        | SystemUnfreezeSegment
        | SystemStatement
        | ConnectbyClause
        | CallSegment
        | WithingroupClause
        | PatternExpression
        | MatchRecognizeClause
        | ChangesClause
        | MatchConditionClause
        | FromAtExpression
        | FromBeforeExpression
        | SnowflakeKeywordExpression
        | SemiStructuredExpression
        | SelectExcludeClause
        | SelectRenameClause
        | AlterTableTableColumnAction
        | AlterTableClusteringAction
        | AlterTableConstraintAction
        | AlterWarehouseStatement
        | AlterShareStatement
        | AlterStorageIntegrationStatement
        | AlterExternalTableStatement
        | CommentEqualsClause
        | TagBracketedEquals
        | TagEquals
        | CreateCloneStatement
        | CreateDatabaseFromShareStatement
        | CreateProcedureStatement
        | ScriptingBlockStatement
        | ScriptingLetStatement
        | AlterFunctionStatement
        | CreateExternalFunctionStatement
        | WarehouseObjectProperties
        | ConstraintPropertiesSegment
        | CopyOptions
        | SchemaObjectProperties
        | CreateTaskStatement
        | SnowflakeTaskExpressionSegment
        | CreateStatement
        | CreateFileFormatSegment
        | AlterFileFormatSegment
        | CsvFileFormatTypeParameters
        | JsonFileFormatTypeParameters
        | AvroFileFormatTypeParameters
        | OrcFileFormatTypeParameters
        | ParquetFileFormatTypeParameters
        | XmlFileFormatTypeParameters
        | AlterPipeSegment
        | FileFormatSegment
        | FormatTypeOptions
        | CopyIntoLocationStatement
        | CopyIntoTableStatement
        | StorageLocation
        | StageParameters
        | S3ExternalStageParameters
        | GcsExternalStageParameters
        | AzureBlobStorageExternalStageParameters
        | CreateStageStatement
        | AlterStageStatement
        | CreateStreamStatement
        | AlterStreamStatement
        | ShowStatement
        | AlterUserStatement
        | AlterSessionStatement
        | AlterSessionSetStatement
        | AlterSessionUnsetClause
        | AlterTaskStatement
        | AlterTaskSpecialSetClause
        | AlterTaskSetClause
        | AlterTaskUnsetClause
        | ExecuteTaskClause
        | UndropStatement
        | CommentStatement
        | DropExternalTableStatement
        | ListStatement
        | GetStatement
        | PutStatement
        | RemoveStatement
        | CastExpression
        | DropObjectStatement
        | UnsetStatement
        | SqlConfOption
        | CreateWidgetStatement
        | ReplaceTableStatement
        | RemoveWidgetStatement
        | UseDatabaseStatement
        | InsertOverwriteDirectoryStatement
        | InsertOverwriteDirectoryHiveFmtStatement
        | LoadDataStatement
        | ClusterByClause
        | DistributeByClause
        | HintFunction
        | SelectHint
        | WithCubeRollupClause
        | SortByClause
        | LateralViewClause
        | PivotClause
        | TransformClause
        | AddFileStatement
        | AddJarStatement
        | AnalyzeTableStatement
        | CacheTable
        | ClearCache
        | ListFileStatement
        | ListJarStatement
        | RefreshStatement
        | UncacheTable
        | FileReference
        | GeneratedColumnDefinition
        | IntervalLiteral
        | DescribeHistoryStatement
        | DescribeDetailStatement
        | GenerateManifestFileStatement
        | ConvertToDeltaStatement
        | RestoreTableStatement
        | ConstraintStatement
        | ApplyChangesIntoStatement
        | UsingClause
        | DataSourceFormat
        | IcebergTransformation
        | MsckRepairTableStatement
        | RowFormatClause
        | SkewedByClause
        | Bracketed
        | EndOfFile
        | Whitespace
        | Newline
        | Unlexable
        | StartBracket
        | EndBracket
        | Raw
        | Dot
        | Comma
        | EmitsSegment
        | Literal
        | Meta
        | Colon
        | StatementTerminator
        | StartSquareBracket
        | EndSquareBracket
        | StartCurlyBracket
        | Word
        | DoubleQuote
        | SingleQuote
        | Semicolon
        | BackQuote
        | DollarQuote
        | Question
        | EndCurlyBracket
        | Dedent
        | Indent
        | Implicit
        | QuestionMark
        | UdfBody
        | StartAngleBracket
        | EndAngleBracket
        | ProcedureOption
        | ExportOption
        | Symbol
        | ExecuteScriptStatement
        | Batch
        | PivotColumnReference
        | IntoTableClause
        | PasswordAuth
        | ExecuteAsClause
        | ExecuteImmediateClause
        | UnicodeSingleQuote
        | EscapedSingleQuote
        | UnicodeDoubleQuote
        | At
        | FileKeyword
        | SemiStructuredElement
        | BytesDoubleQuote
        | BytesSingleQuote
        | FileFormat
        | FileType
        | StartHint
        | EndHint
        | UnquotedFilePath
        | Dollar
        | StageEncryptionOption
        | BucketPath
        | QuotedStar
        | StagePath
        | ColumnSelector
        | ExcludeBracketClose
        | WarehouseSize
        | ExcludeBracketOpen
        | SymlinkFormatManifest
        | StartExcludeBracket
        | CompressionType
        | CopyOnErrorOption
        | ColumnIndexIdentifierSegment
        | ScalingPolicy
        | ValidationModeOption
        | EndExcludeBracket
        | IdentifierList
        | TemplateLoop
        | ColonDelimiter
        | SqlcmdOperator
        | Slice
        | TableEndClauseSegment
        | PragmaStatement
        | PragmaReference
        | Slash
        | DataFormatSegment
        | AuthorizationSegment
        | ColumnAttributeSegment
        | ShowModelStatement
        | CreateExternalSchemaStatement
        | CreateLibraryStatement
        | UnloadStatement
        | DeclareStatement
        | FetchStatement
        | CloseStatement
        | CreateDatashareStatement
        | DescDatashareStatement
        | DropDatashareStatement
        | ShowDatasharesStatement
        | GrantDatashareStatement
        | CreateRlsPolicyStatement
        | ManageRlsPolicyStatement
        | DropRlsPolicyStatement
        | AnalyzeCompressionStatement
        | PartitionedBySegment
        | RowFormatDelimitedSegment
        | ObjectUnpivoting
        | ArrayUnnesting
        | AlterGroup
        | CreateGroup
        | ListaggOverflowClauseSegment
        | UnorderedSelectStatementSegment
        | MapType
        | MapTypeSchema
        | PrepareStatement
        | ExecuteStatement
        | RenameTableStatement
        | ActionParameter
        | AggregateClause
        | AggregateOrderBy
        | AliasOperator
        | AllowConnections
        | AlterAccountStatement
        | AlterAggregateStatement
        | AlterBiCapacityStatement
        | AlterCapacityStatement
        | AlterCatalogStatement
        | AlterDynamicTableStatement
        | AlterEventStatement
        | AlterExtensionStatement
        | AlterExternalVolumeStatement
        | AlterForeignTableActionSegment
        | AlterForeignTableStatement
        | AlterMaskingPolicy
        | AlterMasterKeyStatement
        | AlterNetworkPolicyStatement
        | AlterOptionSegment
        | AlterOrganizationStatement
        | AlterPartitionFunctionStatement
        | AlterPartitionSchemeStatement
        | AlterPasswordPolicyStatement
        | AlterProjectStatement
        | AlterReservationStatement
        | AlterResourceMonitorStatement
        | AlterRowAccessPolicyStatement
        | AlterSecurityPolicyStatement
        | AlterStatisticsStatement
        | AlterStreamlitStatement
        | AlterSubscription
        | AlterTableSwitchStatement
        | AlterTagStatement
        | AlterTextSearchConfigurationStatement
        | AlterVolumeStatement
        | ArnCatalogSchemaSegment
        | ArrayTypeSchema
        | AtSign
        | AtomicBeginEndBlock
        | Atsign
        | AutoOption
        | BackupStorageRedundancy
        | BeginEndBlock
        | BeginStatement
        | BinaryLiteral
        | BindVariable
        | BitValueLiteral
        | BracketedIndexColumnListGrammar
        | BulkInsertStatement
        | BulkInsertWithSegment
        | ByteLengthLiteral
        | CallOperator
        | CatalogReference
        | CharacteristicStatement
        | CheckConstraintGrammar
        | CheckTableStatement
        | ChecksumTableStatement
        | CloseCursorStatement
        | ClusterbyClause
        | Code
        | CollationClause
        | ColonLiteral
        | ColonPrefix
        | ColumnPathOperator
        | ColumnPropertiesSegment
        | ColumnTypeReference
        | ColumnsExpression
        | Command
        | CompatibilityLevel
        | CompositeValueExpansion
        | ComputedColumnDefinition
        | ConflictClause
        | ConnectionConstraintGrammar
        | ConnectionLimitSegment
        | CopyFilesIntoLocationStatement
        | CreateAggregateStatement
        | CreateAssignmentStatement
        | CreateAuthenticationPolicySegment
        | CreateCapacityStatement
        | CreateCatalogStatement
        | CreateColumnstoreIndexStatement
        | CreateCortexSearchServiceStatement
        | CreateDatabaseRoleStatement
        | CreateDatabaseScopedCredentialStatement
        | CreateDatabaseWithOptions
        | CreateEventStatement
        | CreateEventTableStatement
        | CreateExternalDataSourceStatement
        | CreateExternalFileFormat
        | CreateExternalVolumeStatement
        | CreateForeignDataWrapper
        | CreateForeignTableStatement
        | CreateFulltextCatalogStatement
        | CreateFulltextIndexStatement
        | CreateLoginStatement
        | CreateMasterKeyStatement
        | CreateMaterializedViewAsReplicaOfStatement
        | CreateOperatorStatement
        | CreateOptionSegment
        | CreatePartitionFunctionStatement
        | CreatePartitionSchemeStatement
        | CreatePasswordPolicyStatement
        | CreateReservationStatement
        | CreateResourceMonitorStatement
        | CreateRowAccessPolicyStatement
        | CreateSearchIndexStatement
        | CreateSecurityPolicyStatement
        | CreateServerRoleStatement
        | CreateSnapshotTableStatement
        | CreateSqlFunctionStatement
        | CreateStatisticsStatement
        | CreateStreamlitStatement
        | CreateSubscription
        | CreateSynonymStatement
        | CreateTableAsSelectStatement
        | CreateTableFunctionStatement
        | CreateTableGraphStatement
        | CreateTableUsingStatement
        | CreateTextSearchConfigurationStatement
        | CreateTrigger
        | CreateVectorIndexStatement
        | CreateVirtualTableStatement
        | CreateVolumeStatement
        | CursorDefinition
        | CursorFetchSegment
        | CursorOpenCloseSegment
        | DataGovernancePolicyTagActionSegment
        | DatabaseRoleReference
        | DateFormat
        | DeallocateCursorStatement
        | DeallocateSegment
        | DeallocateStatement
        | DeclareOrReplaceVariableStatement
        | DefaultCollate
        | DefinerSegment
        | DeleteTargetTable
        | DelimiterStatement
        | DisableTrigger
        | DistributebyClause
        | DoubleAmpersand
        | DoubleAtSignLiteral
        | DoubleVerticalBar
        | DropAggregateStatement
        | DropAssignmentStatement
        | DropCapacityStatement
        | DropCatalogStatement
        | DropCollationStatement
        | DropColumnClause
        | DropDynamicTableSegment
        | DropEventStatement
        | DropExternalVolumeStatement
        | DropForeignTableStatement
        | DropIcebergTableStatement
        | DropMasterKeyStatement
        | DropModelstatement
        | DropPasswordPolicyStatement
        | DropReservationStatement
        | DropResourceMonitorStatement
        | DropRowAccessPolicyStatement
        | DropSearchIndexStatement
        | DropSecurityPolicy
        | DropStatement
        | DropStatisticsStatement
        | DropSubscription
        | DropSynonymStatement
        | DropTableFunctionStatement
        | DropTextSearchConfigurationStatement
        | DropTrigger
        | DropVectorIndexStatement
        | DropVolumeStatement
        | DynamicTableLagIntervalSegment
        | DynamicTableOptions
        | Edition
        | EncryptedWithGrammar
        | ExceptionBlockStatement
        | ExceptionCode
        | ExecuteArrow
        | ExecuteImmediate
        | ExecuteOption
        | ExecuteSegment
        | ExtendClause
        | ExternalAccessIntegrationEquals
        | ExternalFileDelimitedTextClause
        | ExternalFileDelimitedTextFormatOptionsClause
        | ExternalFileDeltaClause
        | ExternalFileJsonClause
        | ExternalFileOrcClause
        | ExternalFileParquetClause
        | ExternalFileRcClause
        | ExternalLocation
        | ExternalVolumeReference
        | FatRightArrow
        | FetchCursorStatement
        | FileCompression
        | FileEncoding
        | FileSpec
        | FileSpecFileGrowth
        | FileSpecFileName
        | FileSpecMaxSize
        | FileSpecNewName
        | FileSpecSize
        | FileSpecWithoutBracket
        | FilegroupClause
        | FilegroupName
        | FilestreamOnOptionStatement
        | FlushStatement
        | ForSystemTimeAsOfSegment
        | FormatClause
        | FromDatashareClause
        | FromIntegrationClause
        | FullTextSearchOperator
        | FunctionOptionSegment
        | FunctionParameterListWithComments
        | GetDiagnosticsSegment
        | GlobOperator
        | GoStatement
        | GotoStatement
        | GrantToSegment
        | GraphTableConstraint
        | GroupAndOrderbyClause
        | HashIdentifier
        | HashPrefix
        | HelpStatement
        | HexadecimalLiteral
        | IamRoleClause
        | IcebergTableOptions
        | IdentifierClauseSegment
        | IdentityGrammar
        | IfClause
        | IfThenStatement
        | IndexHintClause
        | IndexOption
        | IndexReference
        | IndexType
        | InitializeType
        | InlineDollarSign
        | InlinePathOperator
        | InsertRowAlias
        | IntoOutfileClause
        | IsolationLevelClause
        | IterateStatement
        | JsonPath
        | LabelSegment
        | LambdaArrow
        | LambdaFunction
        | LeadingDot
        | LimitClauseComponent
        | ListComprehension
        | ListaggOverflowClause
        | LocalAliasSegment
        | LogLevelEquals
        | LogicalFileName
        | LoginUserSegment
        | MagicCellSegment
        | MagicLine
        | MagicSingleLine
        | MagicStart
        | MaskStatement
        | MasterKeyEncryptionOption
        | MatchCondition
        | MaxDuration
        | MaxLiteral
        | MetaCommand
        | MetaCommandQueryBuffer
        | MetaCommandStatement
        | MlTableExpression
        | MsckTableStatement
        | NotOperator
        | NotebookStart
        | ObevoAnnotation
        | OffsetClause
        | OnPartitionOrFilegroupStatement
        | OnPartitionsClause
        | OpenCursorStatement
        | OpenSymmetricKeyStatement
        | OpenjsonSegment
        | OpenjsonWithClause
        | OpenquerySegment
        | OpenrowsetSegment
        | OpenrowsetWithClause
        | OpenxmlSegment
        | Operator
        | OptimizeTableStatement
        | Option
        | OptionClause
        | OptionIndicator
        | OutputClause
        | ParameterDirection
        | PartitionClause
        | PartitionSchemeClause
        | PartitionSchemeName
        | PasswordPolicyOptions
        | PasswordPolicyReference
        | PeriodSegment
        | PgTrgmOperator
        | PgvectorOperator
        | PipeOperator
        | PipeOperatorClause
        | PipeStatement
        | PivotOperator
        | PostTableExpression
        | PostgisOperator
        | PrepareSegment
        | PrewhereClause
        | PrintStatement
        | ProcedureStatement
        | PurgeBinaryLogsStatement
        | QualifiedOperator
        | QueryHintSegment
        | QuestionLiteral
        | RaiserrorStatement
        | RawDoubleQuote
        | RawSingleQuote
        | ReconfigureStatement
        | ReferencesConstraintGrammar
        | RelationalIndexOptions
        | RenameColumnClause
        | RenameStatement
        | RepairTableStatement
        | ReplaceStatement
        | ResetMasterStatement
        | ResetSessionAuthorizationStatement
        | ResignalSegment
        | ResourceConstraint
        | ResourceMonitorOptions
        | ReturnSegment
        | ReturningClause
        | SchemaReference
        | ScriptingDeclareStatement
        | ScriptingIfStatement
        | ScriptingRaiseStatement
        | SearchOptimizationAction
        | SecurityLabelStatement
        | SelectVariableAssignment
        | SequenceNextValue
        | SequenceReference
        | SerdeMethod
        | ServiceObjective
        | SetConstraintStatement
        | SetContextInfoStatement
        | SetLanguageStatement
        | SetLocalVariableSegment
        | SetNamesStatement
        | SetOperatorClause
        | SetSessionAuthorizationStatement
        | SetSessionStatement
        | SetTimezoneStatement
        | SetTransactionStatement
        | SetVariableStatement
        | SettingsClause
        | SimplifiedPivot
        | SimplifiedUnpivot
        | SingleQuoteWithN
        | SizeLiteral
        | SortbyClause
        | SqlcmdCommandSegment
        | SquareQuote
        | StatisticsReference
        | StoringSegment
        | SubscriptionReference
        | SynonymReference
        | SystemVariable
        | TableClausesSegment
        | TableClusterByClause
        | TableColumnCommentAction
        | TableDistributionClause
        | TableDistributionIndexClause
        | TableIndexClause
        | TableIndexSegment
        | TableLocationClause
        | TableOptionStatement
        | TableSpecificationSegment
        | TagStatement
        | TemporalQuery
        | TextimageOnOptionStatement
        | ThrowStatement
        | TraceLevelEquals
        | TruncateTable
        | TryCatch
        | TupleTypeSchema
        | UndropSchemaStatement
        | UnpivotMultiColumn
        | UnpivotOperator
        | UnpivotSingleColumn
        | UnquotedRelativeSqlFilePath
        | UpdateStatisticsStatement
        | UpsertClause
        | UpsertClauseList
        | UseCatalogStatement
        | VolumeReference
        | WaitforStatement
        | WildcardExclude
        | WildcardPatternMatching
        | WildcardRename
        | WildcardReplace
        | WithCheckOptions
        | WithFill
        | WithRollupClause
        | WithinGroupClause
        | WithordinalityClause
        | OracleAtSign
        | OraclePowerOperator
        | OracleAssignmentOperator
        | HierarchicalQueryClause
        | PivotSegment
        | UnpivotSegment
        | TriggerCorrelationName
        | OracleBindVariable
        | AlterTableProperties
        | AlterTableColumnClauses
        | AlterTableConstraintClauses
        | IndexTypeReference
        | ExecuteFileStatement
        | SlashBufferExecutor
        | OracleBatch
        | OracleCommentStatement
        | OracleCreateProcedureStatement
        | OracleDropProcedureStatement
        | OracleDeclareSegment
        | OracleColumnTypeReference
        | OracleRowTypeReference
        | CollectionType
        | RecordType
        | RefCursorType
        | DeclareCursorVariable
        | OracleExecuteImmediateStatement
        | OracleBeginEndBlock
        | OracleCreateFunctionStatement
        | OracleAlterFunctionStatement
        | OracleCreateTypeStatement
        | OracleTypeReference
        | OracleCreateTypeBodyStatement
        | OracleCreatePackageStatement
        | OraclePackageReference
        | OracleAlterPackageStatement
        | OracleDropPackageStatement
        | OracleCreateTriggerStatement
        | DmlEventClause
        | OracleReferencingClause
        | CompoundTriggerStatement
        | TimingPointSection
        | OracleAlterTriggerStatement
        | AssignmentSegmentStatement
        | OracleIfThenStatement
        | OracleIfClause
        | OracleCaseExpression
        | OracleWhenClause
        | OracleElseClause
        | OracleNullStatement
        | ForLoopStatement
        | WhileLoopStatement
        | OracleLoopStatement
        | ForallStatement
        | OracleOpenStatement
        | OracleOpenForStatement
        | OracleFetchStatement
        | OracleIntoClause
        | BulkCollectIntoClause
        | OracleExitStatement
        | OracleReturnStatement
        | OracleCreateUserStatement
        | OracleReturningClause
        | DatabaseLinkReference
        | OracleCreateDatabaseLinkStatement
        | OracleDropDatabaseLinkStatement
        | OracleAlterDatabaseLinkStatement
        | OracleCreateSynonymStatement
        | OracleDropSynonymStatement
        | OracleAlterSynonymStatement
        | OracleWithinGroupClause
        | OracleListaggOverflowClause
        | OracleNamedArgument
        | OracleCreateTableStatement
        | OracleColumnDefinition
        | OracleSqlplusVariable
        | StartwithClause
        | OracleTableReference
        | OracleCreateViewStatement
        | OracleAlterIndexStatement
        | OracleAlterTableStatement
        | OracleAlterSessionStatement
        | JsonTableColumnDefinition
        | JsonTableColumnsClause
        | JsonTableFunctionContents
        | OracleMergeUpdateClause
        | OracleInsertStatement
        | OracleUpdateStatement
        | OracleDeleteStatement
        | OracleTransactionStatement
        | OracleValuesClause
        | OracleTableConstraint
        | OracleFunctionName
        | OracleOrderByClause
        | EngineType
        | PartitionSegment
        | DistributionSegment
        | IndexDefinition
        | CreateRoutineLoadStatement
        | RoutineLoadProperties
        | RoutineLoadDataSourceProperties
        | StopRoutineLoadStatement
        | PauseRoutineLoadStatement
        | ResumeRoutineLoadStatement
        | InsertOverwriteStatement => return None,
    };

    Some(highlight)
}

/// Parse `source` with `linter` and produce LSP semantic tokens for it.
///
/// Returns an empty list if the source fails to parse.
pub fn semantic_tokens(
    linter: &Linter,
    source: &str,
    filename: Option<String>,
) -> Vec<SemanticToken> {
    let tables = Tables::default();
    match linter.parse_string(&tables, source, filename) {
        Ok(parsed) => parsed
            .tree
            .map(|tree| build_semantic_tokens(&tree, source))
            .unwrap_or_default(),
        Err(e) => {
            eprintln!("Failed to parse for semantic tokens: {}", e.value);
            Vec::new()
        }
    }
}

/// Build delta-encoded LSP semantic tokens from a parsed `tree` over `source`.
pub(crate) fn build_semantic_tokens(tree: &ErasedSegment, source: &str) -> Vec<SemanticToken> {
    let line_index = LineIndex::new(source);
    let mut tokens: Vec<RawToken> = Vec::new();

    for segment in tree.get_raw_segments() {
        let Some(highlight) = classify(segment.get_type()) else {
            continue;
        };
        let Some(marker) = segment.get_position_marker() else {
            continue;
        };

        let range = marker.source_slice.clone();
        if range.is_empty() || range.end > source.len() {
            continue;
        }
        let text = &source[range.clone()];
        let token_type = highlight.token_type();

        // The LSP wire format cannot represent a token spanning multiple lines,
        // so split block comments / multi-line strings into one token per line.
        line_index.split_lines(range.start, text, |line, start, length| {
            tokens.push(RawToken {
                line,
                start,
                length,
                token_type,
            });
        });
    }

    // The CST walk yields source order, but the multi-line split can interleave,
    // so re-sort before delta encoding.
    tokens.sort_by_key(|t| (t.line, t.start));

    let mut data = Vec::with_capacity(tokens.len());
    let mut prev_line = 0;
    let mut prev_start = 0;
    for token in tokens {
        let delta_line = token.line - prev_line;
        let delta_start = if delta_line == 0 {
            token.start - prev_start
        } else {
            token.start
        };
        data.push(SemanticToken {
            delta_line,
            delta_start,
            length: token.length,
            token_type: token.token_type,
            token_modifiers_bitset: 0,
        });
        prev_line = token.line;
        prev_start = token.start;
    }

    data
}

/// A token before delta encoding: absolute line + UTF-16 start column + length.
struct RawToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
}

/// Maps byte offsets in the source to LSP `(line, character)` positions, where
/// `character` is counted in UTF-16 code units (the LSP default encoding).
struct LineIndex<'a> {
    source: &'a str,
    /// Byte offset of the start of each line.
    line_starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    fn new(source: &'a str) -> Self {
        let mut line_starts = vec![0];
        for (offset, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(offset + 1);
            }
        }
        Self {
            source,
            line_starts,
        }
    }

    /// Resolve a byte offset to `(line, utf16_column)`.
    fn position(&self, offset: usize) -> (u32, u32) {
        let line = match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next) => next - 1,
        };
        let line_start = self.line_starts[line];
        let column = utf16_len(&self.source[line_start..offset]);
        (line as u32, column)
    }

    /// Split `text` (which starts at byte `start` in the source) into one entry
    /// per line, invoking `emit(line, start_col, len)` with UTF-16 columns and
    /// lengths. Trailing newline / carriage-return characters are excluded.
    fn split_lines(&self, start: usize, text: &str, mut emit: impl FnMut(u32, u32, u32)) {
        let mut offset = start;
        for piece in text.split_inclusive('\n') {
            // Strip the line terminator (`\n`, and a preceding `\r` if present).
            let content = piece.strip_suffix('\n').unwrap_or(piece);
            let content = content.strip_suffix('\r').unwrap_or(content);
            if !content.is_empty() {
                let (line, column) = self.position(offset);
                emit(line, column, utf16_len(content));
            }
            offset += piece.len();
        }
    }
}

/// Count the number of UTF-16 code units in `text`.
fn utf16_len(text: &str) -> u32 {
    text.chars().map(|c| c.len_utf16() as u32).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqruff_lib::core::config::FluffConfig;

    fn linter() -> Linter {
        let config = FluffConfig::from_source("[sqruff]\ndialect = ansi\n", None);
        Linter::new(config, None, None, false).unwrap()
    }

    /// Decode the delta-encoded wire format back into absolute
    /// `(line, start, length, token_type)` tuples for readable assertions.
    fn decode(tokens: &[SemanticToken]) -> Vec<(u32, u32, u32, u32)> {
        let mut line = 0;
        let mut start = 0;
        let mut out = Vec::new();
        for token in tokens {
            if token.delta_line == 0 {
                start += token.delta_start;
            } else {
                line += token.delta_line;
                start = token.delta_start;
            }
            out.push((line, start, token.length, token.token_type));
        }
        out
    }

    fn tokens(sql: &str) -> Vec<(u32, u32, u32, u32)> {
        decode(&semantic_tokens(&linter(), sql, None))
    }

    #[test]
    fn highlights_keywords_identifiers_and_literals() {
        // SELECT a, 1, 'x' FROM t
        let decoded = tokens("SELECT a, 1, 'x' FROM t");
        let kw = Highlight::Keyword.token_type();
        let var = Highlight::Variable.token_type();
        let num = Highlight::Number.token_type();
        let string = Highlight::String.token_type();

        assert_eq!(
            decoded,
            vec![
                (0, 0, 6, kw),      // SELECT
                (0, 7, 1, var),     // a
                (0, 10, 1, num),    // 1
                (0, 13, 3, string), // 'x'
                (0, 17, 4, kw),     // FROM
                (0, 22, 1, var),    // t
            ]
        );
    }

    #[test]
    fn skips_whitespace_and_punctuation() {
        // Commas / whitespace must not produce tokens.
        let decoded = tokens("SELECT a, b");
        assert!(decoded.iter().all(|&(_, _, len, _)| len > 0));
        // SELECT, a, b => exactly three tokens.
        assert_eq!(decoded.len(), 3);
    }

    #[test]
    fn splits_block_comment_across_lines() {
        let decoded = tokens("SELECT 1 /* line one\nline two */ FROM t");
        let comment = Highlight::Comment.token_type();
        let comment_tokens: Vec<_> = decoded
            .iter()
            .filter(|&&(_, _, _, ty)| ty == comment)
            .collect();
        // The block comment spans two lines => two comment tokens, one per line.
        assert_eq!(comment_tokens.len(), 2);
        assert_eq!(comment_tokens[0].0, 0);
        assert_eq!(comment_tokens[1].0, 1);
    }

    #[test]
    fn utf16_columns_after_non_ascii() {
        // A non-ASCII char in a string shifts later columns; ensure we count
        // UTF-16 code units, not bytes.
        let sql = "SELECT 'é' AS a";
        let decoded = tokens(sql);
        let var = Highlight::Variable.token_type();
        // `a` is the alias identifier; in UTF-16 it sits at column 14.
        // S E L E C T _ ' é ' _ A S _ a
        // 0 1 2 3 4 5 6 7 8 9 ...
        let alias = decoded
            .iter()
            .rev()
            .find(|&&(_, _, _, ty)| ty == var)
            .unwrap();
        assert_eq!(alias.1, 14);
    }
}
