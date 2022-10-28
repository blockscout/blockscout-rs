import React, { ChangeEvent, FormEvent, FormEventHandler } from 'react';

import { Input, InputGroup, InputLeftAddon, InputRightElement } from '@chakra-ui/input';
import { chakra, IconButton } from '@chakra-ui/react';
import { useColorModeValue } from '@chakra-ui/color-mode';

import  { SearchIcon } from '@chakra-ui/icons'

import styles from '../styles/search.module.css'

interface Props {
  initialValue: string
}

export const SearchBar = ({initialValue}: Props) => {
  const [ value, setValue ] = React.useState(initialValue);

  const onChange = React.useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setValue(event.target.value);
  }, []);

  const onSubmit = React.useCallback((event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (value) {
      const url = 'search_results?q=' + value;
      window.location.assign(url);
    }
  }, [ value ]);

  return (
    <chakra.form noValidate onSubmit={ onSubmit } display={{ base: 'none', lg: 'block' }} w="100%" className={styles.search}>
    <InputGroup size='md'>
      <InputLeftAddon color="#5A349C" fontWeight="bold" fontSize={16}>
        Search in chains
      </InputLeftAddon>
      <Input
        placeholder="by address / transaction / block / token... "
        ml="1px"
        onChange={ onChange }
        borderColor={ useColorModeValue('blackAlpha.100', 'whiteAlpha.200') }
        defaultValue={initialValue}
      />
      <InputRightElement>
        <IconButton size="sm" aria-label='search' icon={<SearchIcon/>} type="submit"></IconButton>
      </InputRightElement>
    </InputGroup>
    </chakra.form>

  );
};

